//! BLiMS Car Test
//!
//! PURPOSE:
//! Tests the full BLiMS LV flight logic by calling new() once then execute()
//! at 20 Hz (50 ms cycle), exactly as FSW does in MainDeployedMode.
//! Real GPS provides lat/lon/heading; simulated altitude from L3 Launch 4
//! descent data drives phase transitions through the landing pattern.
//!
//! FSW PATTERN THIS REPLICATES:
//!   StartupMode::execute()      → Blims::new(...)
//!   MainDeployedMode::execute() → pack BlimsDataIn, call blims.execute()
//!   flight_loop                 → sleep remaining cycle time (50 ms target)
//!
//! NOTE — FSW BUG (same as C++ version):
//! Make sure altitude_ft is populated in BlimsDataIn before flight.
//! It defaults to 0, meaning BLiMS always sees Phase::Neutral otherwise.
//!   data_in.altitude_ft = barometer_altitude_m * 3.28084;
//!
//! WIRING:
//!   PWM    → GPIO 28     ODrive enable → GPIO 0
//!   SDA    → GPIO 12     SCL           → GPIO 13   (I2C0, 400 kHz)
//!
//! OUTPUT CSV (13 fields, compatible with car_test_visualizer.py):
//!   lat,lon,target_lat,target_lon,heading,bearing,motor_pos,
//!   timestamp_ms,P,I,phase,altitude,loiter_step

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cIrqHandler};
use embassy_rp::peripherals::I2C0;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Instant, Timer};
use fixed::types::extra::U4;
use fixed::FixedU16;
use {defmt_rtt as _, panic_probe as _};

use blims::blims_constants::*;
use blims::blims_state::BlimsDataIn;
use blims::Blims;

// ============================================================================
// INTERRUPT BINDING — module level, not inside a function
// ============================================================================

embassy_rp::bind_interrupts!(struct Irqs {
    I2C0_IRQ => I2cIrqHandler<I2C0>;
});

// ============================================================================
// PIN DEFINITIONS
// ============================================================================

const PWM_PIN_NUM:    u8 = 28;
const ENABLE_PIN_NUM: u8 = 0;
const I2C_SDA_NUM:    u8 = 12;
const I2C_SCL_NUM:    u8 = 13;

// ============================================================================
// TEST CONFIGURATION — update before each test
// ============================================================================

const TARGET_LAT: f32 = 42.446610;
const TARGET_LON: f32 = -76.461304;

/// 50 ms = 20 Hz, matches FSW constants::cycle_time
const CYCLE_TIME_MS: u64 = 50;

// ============================================================================
// WIND PROFILE
// Altitude in metres AGL, wind direction in degrees (coming FROM).
// Update with real sounding data or forecast before each test/flight.
// ============================================================================

const WIND_PROFILE_SIZE: usize = 11;
const WIND_ALTITUDES_M: [f32; WIND_PROFILE_SIZE] =
    [0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 500.0, 550.0, 610.0];
const WIND_DIRS_DEG: [f32; WIND_PROFILE_SIZE] =
    [45.0, 48.0, 52.0, 56.0, 60.0, 64.0, 68.0, 75.0, 80.0, 85.0, 90.0];
/// Fallback single wind value used if no profile is loaded
const WIND_FROM_DEG: f32 = 45.0;

// ============================================================================
// SIMULATED DESCENT DATA (mirrors descent_alt_data.hpp)
// Replace make_descent_data() with your actual L3 Launch 4 altitude array.
// 200 samples × 50 ms = 10 seconds, 1500 ft → 0 ft
// ============================================================================

const DESCENT_DATA_SIZE: usize = 200;

const fn make_descent_data() -> [f32; DESCENT_DATA_SIZE] {
    let mut data = [0.0f32; DESCENT_DATA_SIZE];
    let mut i = 0usize;
    while i < DESCENT_DATA_SIZE {
        data[i] = 1500.0 - (1500.0 / DESCENT_DATA_SIZE as f32) * i as f32;
        i += 1;
    }
    data
}

static DESCENT_ALT_FT: [f32; DESCENT_DATA_SIZE] = make_descent_data();

// ============================================================================
// HELPERS
// ============================================================================

fn phase_name(phase_id: i8) -> &'static str {
    match phase_id {
        0 => "HELD",
        1 => "TRACK",
        2 => "DOWNWIND",
        3 => "BASE",
        4 => "FINAL",
        5 => "NEUTRAL",
        6 => "LOITER",
        _ => "???",
    }
}

// ============================================================================
// U-BLOX GPS — minimal UBX-NAV-PVT I2C driver
// Mirrors ublox_mx / ublox_nav_pvt from the C++ version.
// ============================================================================

const UBLOX_ADDR:     u8 = 0x42;
const UBX_CLASS_NAV:  u8 = 0x01;
const UBX_ID_PVT:     u8 = 0x07;
const UBX_CLASS_CFG:  u8 = 0x06;
const UBX_ID_RATE:    u8 = 0x08;
const NAV_PVT_LEN: usize = 100;

/// Mirrors UbxNavPvt from ublox_nav_pvt.hpp
#[derive(Default, Clone)]
struct UbxNavPvt {
    fix_type: u8,
    lat:      i32,  // degrees × 1e7
    lon:      i32,  // degrees × 1e7
    h_acc:    u32,  // mm
    v_acc:    u32,  // mm
    vel_n:    i32,  // mm/s
    vel_e:    i32,  // mm/s
    vel_d:    i32,  // mm/s  positive = descending
    g_speed:  i32,  // mm/s
    head_mot: i32,  // degrees × 1e5
    s_acc:    u32,  // mm/s
    head_acc: u32,  // degrees × 1e5
}

/// Compute UBX Fletcher checksum over payload bytes (after sync chars)
fn ubx_checksum(msg: &[u8]) -> (u8, u8) {
    let mut a: u8 = 0;
    let mut b: u8 = 0;
    for &byte in msg {
        a = a.wrapping_add(byte);
        b = b.wrapping_add(a);
    }
    (a, b)
}

/// Configure GPS output rate to 20 Hz via UBX-CFG-RATE.
/// Mirrors gps.begin_PVT(20) from the C++ version.
async fn gps_set_rate_20hz(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) -> bool {
    // UBX-CFG-RATE: measRate=50ms (20Hz), navRate=1, timeRef=1 (GPS)
    // Full frame built manually — no heap/.concat() needed in no_std
    let mut frame = [0u8; 14]; // sync(2) + class+id+len(4) + payload(6) + ck(2)
    frame[0]  = 0xB5;
    frame[1]  = 0x62;
    frame[2]  = UBX_CLASS_CFG;
    frame[3]  = UBX_ID_RATE;
    frame[4]  = 0x06; // payload length (6), low byte
    frame[5]  = 0x00; // payload length high byte
    frame[6]  = 0x32; // measRate = 50 ms, low byte
    frame[7]  = 0x00; // measRate high byte
    frame[8]  = 0x01; // navRate = 1, low byte
    frame[9]  = 0x00;
    frame[10] = 0x01; // timeRef = 1 (GPS), low byte
    frame[11] = 0x00;
    // Checksum covers bytes 2..12 (class through end of payload)
    let (ck_a, ck_b) = ubx_checksum(&frame[2..12]);
    frame[12] = ck_a;
    frame[13] = ck_b;

    i2c.write_async(UBLOX_ADDR, frame).await.is_ok()
}
/// Poll GPS for a fresh NAV-PVT packet.
/// Returns None if the I2C transaction fails or the response is invalid.
async fn read_nav_pvt(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) -> Option<UbxNavPvt> {
    // Build UBX poll request: sync + class + id + length(0,0) + checksum
    let mut poll_msg = [0u8; 4];
    poll_msg[0] = UBX_CLASS_NAV;
    poll_msg[1] = UBX_ID_PVT;
    poll_msg[2] = 0x00;
    poll_msg[3] = 0x00;
    let (ck_a, ck_b) = ubx_checksum(&poll_msg);

    let poll: [u8; 8] = [
        0xB5, 0x62,
        UBX_CLASS_NAV, UBX_ID_PVT,
        0x00, 0x00,
        ck_a, ck_b,
    ];

    i2c.write_async(UBLOX_ADDR, poll).await.ok()?;

    // Short gap before reading — u-blox recommended polling delay
    Timer::after(Duration::from_millis(10)).await;

    // 6-byte UBX header + 100-byte payload + 2-byte checksum
    let mut buf = [0u8; 6 + NAV_PVT_LEN + 2];
    i2c.read_async(UBLOX_ADDR, &mut buf).await.ok()?;

    // Validate sync chars and message identity
    if buf[0] != 0xB5
        || buf[1] != 0x62
        || buf[2] != UBX_CLASS_NAV
        || buf[3] != UBX_ID_PVT
    {
        return None;
    }

    // Payload starts at byte 6.
    // Field offsets from u-blox MAX-M10 datasheet (UBX-NAV-PVT):
    //   20  fixType  u1
    //   28  lon      i4   deg × 1e-7
    //   32  lat      i4   deg × 1e-7
    //   40  hAcc     u4   mm
    //   44  vAcc     u4   mm
    //   48  velN     i4   mm/s
    //   52  velE     i4   mm/s
    //   56  velD     i4   mm/s
    //   60  gSpeed   i4   mm/s
    //   64  headMot  i4   deg × 1e-5
    //   68  sAcc     u4   mm/s
    //   72  headAcc  u4   deg × 1e-5
    let p = &buf[6..6 + NAV_PVT_LEN];

    Some(UbxNavPvt {
        fix_type: p[20],
        lon:      i32::from_le_bytes([p[28], p[29], p[30], p[31]]),
        lat:      i32::from_le_bytes([p[32], p[33], p[34], p[35]]),
        h_acc:    u32::from_le_bytes([p[40], p[41], p[42], p[43]]),
        v_acc:    u32::from_le_bytes([p[44], p[45], p[46], p[47]]),
        vel_n:    i32::from_le_bytes([p[48], p[49], p[50], p[51]]),
        vel_e:    i32::from_le_bytes([p[52], p[53], p[54], p[55]]),
        vel_d:    i32::from_le_bytes([p[56], p[57], p[58], p[59]]),
        g_speed:  i32::from_le_bytes([p[60], p[61], p[62], p[63]]),
        head_mot: i32::from_le_bytes([p[64], p[65], p[66], p[67]]),
        s_acc:    u32::from_le_bytes([p[68], p[69], p[70], p[71]]),
        head_acc: u32::from_le_bytes([p[72], p[73], p[74], p[75]]),
    })
}

/// Scan I2C bus and print found devices — mirrors the C++ debug scan.
async fn i2c_scan(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) {
    defmt::println!("# I2C scan on I2C0:");
    let mut addr: u8 = 0x08;
    while addr < 0x78 {
        let mut dummy = [0u8; 1];
        if i2c.read_async(addr, &mut dummy).await.is_ok() {
            defmt::println!("#   Found device at 0x{=u8:02X}", addr);
        }
        addr += 1;
    }
    defmt::println!("# I2C scan complete");
}

// ============================================================================
// MAIN
// ============================================================================

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // ── 2-second startup delay (mirrors stdio_init_all + sleep_ms(2000)) ─────
    Timer::after(Duration::from_secs(2)).await;

    // ── I2C0 (SDA=GPIO12, SCL=GPIO13, 400 kHz) ───────────────────────────────
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 400_000;
    let mut i2c = I2c::new_async(p.I2C0, p.PIN_13, p.PIN_12, Irqs, i2c_config);

    // ── PWM (GPIO28, slice 6) — matches working odrive_motor_test/main.rs ─────
    // WRAP_CYCLE_COUNT=65535, DIVIDER=45.78 → 50 Hz on RP2350 at 150 MHz
    let mut pwm_config = PwmConfig::default();
    pwm_config.top      = WRAP_CYCLE_COUNT;
    pwm_config.divider  = FixedU16::<U4>::from_num(45.78_f32);
    pwm_config.compare_a = 0;
    pwm_config.enable   = true;
    let pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone());

    // ── Enable pin (GPIO0) ────────────────────────────────────────────────────
    // Blims::new() takes ownership and drives it HIGH immediately.
    // The C++ car test also pulses it each cycle — we expose it via a wrapper
    // by calling blims.enable_pulse() if needed. For now Blims keeps it HIGH.
    let enable_pin = Output::new(p.PIN_0, Level::Low);

    // ── BLiMS init (mirrors FSW StartupMode::execute) ─────────────────────────
    // Blims::new() drives enable_pin HIGH and parks motor at NEUTRAL_POS (0.5)
    let mut blims = Blims::new(pwm, pwm_config, enable_pin);
    blims.set_target(TARGET_LAT, TARGET_LON);
    blims.set_wind_from_deg(WIND_FROM_DEG);
    blims.set_wind_profile(&WIND_ALTITUDES_M, &WIND_DIRS_DEG);

    // 5-second stabilisation delay (mirrors C++ sleep_ms(5000) after begin)
    Timer::after(Duration::from_secs(5)).await;

    // ── GPS init at 20 Hz (mirrors gps.begin_PVT(20)) ────────────────────────
    if !gps_set_rate_20hz(&mut i2c).await {
        defmt::println!("# ERROR: GPS rate config failed");
        // Continue anyway — GPS may already be running at the right rate
    } else {
        defmt::println!("# GPS configured at 20 Hz");
    }

    // ── I2C bus scan (debug) ──────────────────────────────────────────────────
    i2c_scan(&mut i2c).await;

    // ── Banner ────────────────────────────────────────────────────────────────
    defmt::println!("# ================================================");
    defmt::println!("# BLiMS Car Test (FSW begin/execute pattern)");
    defmt::println!("# ================================================");
    defmt::println!("# Target:   {=f32}, {=f32}", TARGET_LAT, TARGET_LON);
    defmt::println!("# Wind:     {} layers, surface {} deg",
        WIND_PROFILE_SIZE, WIND_DIRS_DEG[0] as u32);
    defmt::println!("# Descent:  {} samples, {=f32} -> {=f32} ft",
        DESCENT_DATA_SIZE,
        DESCENT_ALT_FT[0],
        DESCENT_ALT_FT[DESCENT_DATA_SIZE - 1]);
    defmt::println!("# Cycle:    {} ms (20 Hz)", CYCLE_TIME_MS);
    defmt::println!("# Pins:     PWM={} EN={} SDA={} SCL={}",
        PWM_PIN_NUM, ENABLE_PIN_NUM, I2C_SDA_NUM, I2C_SCL_NUM);
    defmt::println!("# ================================================");
    defmt::println!("# CSV: lat,lon,target_lat,target_lon,heading,bearing,motor_pos,timestamp_ms,P,I,phase,altitude,loiter_step");
    defmt::println!("# ================================================");
    defmt::println!("# Waiting for GPS fix to start descent...");

    // ── Loop state ────────────────────────────────────────────────────────────
    let mut pvt             = UbxNavPvt::default();
    let mut alt_index       = 0usize;
    let mut descent_started = false;
    let mut last_phase_id: i8 = -1;

    // =========================================================================
    // MAIN LOOP — FSW-style 20 Hz cycle
    //
    // Mirrors FSW flight_loop.cpp:
    //   cycle_start = now
    //   mode->execute()   ← sensor reads + blims.execute()
    //   mode->transition()
    //   sleep(cycle_time - elapsed)
    // =========================================================================
    loop {
        let cycle_start = Instant::now();

        // ── 1. Read GPS ───────────────────────────────────────────────────────
        if let Some(new_pvt) = read_nav_pvt(&mut i2c).await {
            pvt = new_pvt;
        }

        let gps_valid = pvt.fix_type >= 2;

        // ── 2. Altitude simulation ────────────────────────────────────────────
        // Descent starts on first valid GPS fix (mirrors C++ logic exactly)
        if !descent_started && gps_valid {
            descent_started = true;
            defmt::println!("# DESCENT STARTED — alt {=f32} ft (0/{})",
                DESCENT_ALT_FT[0],
                DESCENT_DATA_SIZE);
        }

        let current_alt_ft = DESCENT_ALT_FT[alt_index]; // stays at [0] until descent_started

        // ── 3. Pack BlimsDataIn (mirrors FSW MainDeployedMode exactly) ────────
        let data_in = BlimsDataIn {
            lon:         pvt.lon,
            lat:         pvt.lat,
            altitude_ft: if descent_started { current_alt_ft } else { DESCENT_ALT_FT[0] },
            h_acc:       pvt.h_acc,
            v_acc:       pvt.v_acc,
            vel_n:       pvt.vel_n,
            vel_e:       pvt.vel_e,
            vel_d:       pvt.vel_d,
            g_speed:     pvt.g_speed,
            head_mot:    pvt.head_mot,
            s_acc:       pvt.s_acc,
            head_acc:    pvt.head_acc,
            fix_type:    pvt.fix_type,
            gps_state:   gps_valid,
        };

        // ── 4. Execute BLiMS ──────────────────────────────────────────────────
        let data_out = blims.execute(&data_in);

        // ── 5. Advance altitude one sample per cycle ──────────────────────────
        // Runs through all data to ground, then stays at last value
        if descent_started && alt_index < DESCENT_DATA_SIZE - 1 {
            alt_index += 1;
        }

        // ── 6. Log ────────────────────────────────────────────────────────────

        // Phase-change banner (mirrors C++ PHASE_NAMES log)
        if data_out.phase_id != last_phase_id {
            defmt::println!("# PHASE: {} (alt={=f32} ft, sample {}/{})",
                phase_name(data_out.phase_id),
                current_alt_ft,
                alt_index,
                DESCENT_DATA_SIZE);
            last_phase_id = data_out.phase_id;
        }

        // CSV row — only when GPS has a valid fix (mirrors C++ if fixType >= 2)
        if pvt.fix_type >= 2 {
            let heading_deg = pvt.head_mot as f32 * 1e-5;
            let lat_f       = pvt.lat as f32 * 1e-7;
            let lon_f       = pvt.lon as f32 * 1e-7;
            let now_ms      = Instant::now().as_millis();

            // 13 fields — matches car_test_visualizer.py column order exactly
            defmt::println!(
                "{=f32},{=f32},{=f32},{=f32},{=f32},{=f32},{=f32},{=u64},{=f32},{=f32},{=i8},{=f32},{=i8}",
                lat_f,
                lon_f,
                TARGET_LAT,
                TARGET_LON,
                heading_deg,
                data_out.bearing,
                data_out.motor_position,
                now_ms,
                data_out.pid_p,
                data_out.pid_i,
                data_out.phase_id,
                current_alt_ft,
                data_out.loiter_step,
            );
        } else {
            defmt::println!("# No fix (type={})", pvt.fix_type);
        }

        // ── 7. Cycle timing (mirrors FSW flight_loop.cpp) ─────────────────────
        let elapsed_ms = cycle_start.elapsed().as_millis();
        if elapsed_ms < CYCLE_TIME_MS {
            Timer::after(Duration::from_millis(CYCLE_TIME_MS - elapsed_ms)).await;
        } else {
            defmt::println!("# WARN: cycle overrun {} ms", elapsed_ms);
        }
    }
}