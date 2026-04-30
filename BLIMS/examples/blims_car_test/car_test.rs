//! BLiMS Car Test
//!
//! PURPOSE:
//! Tests the full BLiMS LV flight logic by calling new() once then execute()
//! at 20 Hz (50 ms cycle), exactly as FSW does in MainDeployedMode.
//! Real GPS provides lat/lon/heading; altitude from descent_alt_data.rs
//! (L3 Launch 4 real descent data) drives phase transitions.
//!
//! FSW PATTERN THIS REPLICATES:
//!   StartupMode::execute()      → Blims::new(...)
//!   MainDeployedMode::execute() → pack BlimsDataIn, call blims.execute()
//!   flight_loop                 → sleep remaining cycle time (50 ms target)
//!
//! WIRING:
//!   PWM    → GPIO 28     ODrive enable → GPIO 0
//!   SDA    → GPIO 12     SCL           → GPIO 13   (I2C0, 400 kHz)
//!
//! SERIAL OUTPUT (USB CDC-ACM, open with any serial terminal at any baud):
//!   Comment lines begin with '#'.
//!   CSV lines (one per 20 Hz cycle when GPS fix is valid):
//!     lat,lon,target_lat,target_lon,heading,bearing,motor_pos,
//!     timestamp_ms,P,I,phase,altitude,loiter_step

#![no_std]
#![no_main]

//mod descent_alt_data; - no alt data for this car test
//use descent_alt_data::{DESCENT_ALT_FT, DESCENT_DATA_SIZE};

use core::fmt::Write as FmtWrite;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cIrqHandler};
use embassy_rp::peripherals::{I2C0, USB};
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_rp::usb::{Driver, InterruptHandler as UsbIrqHandler};
use embassy_time::{Duration, Instant, Timer};
use fixed::types::extra::U4;
use fixed::FixedU16;
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

use blims::blims_constants::*;
use blims::blims_state::BlimsDataIn;
use blims::{Blims, Hardware};

// ============================================================================
// INTERRUPT BINDINGS
// ============================================================================

bind_interrupts!(struct Irqs {
    I2C0_IRQ    => I2cIrqHandler<I2C0>;
    USBCTRL_IRQ => UsbIrqHandler<USB>;
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

/// 50 ms = 20 Hz, matches FSW cycle_time
const CYCLE_TIME_MS: u64 = 50;

// ============================================================================
// WIND PROFILE
// ============================================================================

const WIND_PROFILE_SIZE: usize = 11;
const WIND_ALTITUDES_M: [f32; WIND_PROFILE_SIZE] =
    [0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 500.0, 550.0, 610.0];
const WIND_DIRS_DEG: [f32; WIND_PROFILE_SIZE] =
    [45.0, 48.0, 52.0, 56.0, 60.0, 64.0, 68.0, 75.0, 80.0, 85.0, 90.0];
const WIND_FROM_DEG: f32 = 45.0;

// ============================================================================
// USB LOGGER TASK
// ============================================================================

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

// ============================================================================
// HELPERS
// ============================================================================

fn phase_name(phase_id: i8) -> &'static str {
    match phase_id {
        0 => "HELD",
        1 => "INITIAL_HOLD",
        2 => "UPWIND",
        3 => "DOWNWIND",
        4 => "NEUTRAL",
        _ => "???",
    }
}

// ============================================================================
// U-BLOX GPS — minimal UBX-NAV-PVT I2C driver
// ============================================================================

const UBLOX_ADDR:    u8 = 0x42;
const UBX_CLASS_NAV: u8 = 0x01;
const UBX_ID_PVT:    u8 = 0x07;
const UBX_CLASS_CFG: u8 = 0x06;
const UBX_ID_RATE:   u8 = 0x08;
const NAV_PVT_LEN: usize = 100;

#[derive(Default, Clone)]
struct UbxNavPvt {
    fix_type: u8,
    lat:      i32,
    lon:      i32,
    h_acc:    u32,
    v_acc:    u32,
    vel_n:    i32,
    vel_e:    i32,
    vel_d:    i32,
    g_speed:  i32,
    head_mot: i32,
    s_acc:    u32,
    head_acc: u32,
}

fn ubx_checksum(msg: &[u8]) -> (u8, u8) {
    let mut a: u8 = 0;
    let mut b: u8 = 0;
    for &byte in msg {
        a = a.wrapping_add(byte);
        b = b.wrapping_add(a);
    }
    (a, b)
}

async fn gps_set_rate_20hz(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) -> bool {
    let mut frame = [0u8; 14];
    frame[0]  = 0xB5;
    frame[1]  = 0x62;
    frame[2]  = UBX_CLASS_CFG;
    frame[3]  = UBX_ID_RATE;
    frame[4]  = 0x06;
    frame[5]  = 0x00;
    frame[6]  = 0x32; // measRate = 50 ms (20 Hz)
    frame[7]  = 0x00;
    frame[8]  = 0x01; // navRate = 1
    frame[9]  = 0x00;
    frame[10] = 0x01; // timeRef = GPS
    frame[11] = 0x00;
    let (ck_a, ck_b) = ubx_checksum(&frame[2..12]);
    frame[12] = ck_a;
    frame[13] = ck_b;
    i2c.write_async(UBLOX_ADDR, frame).await.is_ok()
}

async fn read_nav_pvt(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) -> Option<UbxNavPvt> {
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
    Timer::after(Duration::from_millis(10)).await;

    let mut buf = [0u8; 6 + NAV_PVT_LEN + 2];
    i2c.read_async(UBLOX_ADDR, &mut buf).await.ok()?;

    if buf[0] != 0xB5
        || buf[1] != 0x62
        || buf[2] != UBX_CLASS_NAV
        || buf[3] != UBX_ID_PVT
    {
        return None;
    }

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

async fn i2c_scan(i2c: &mut I2c<'_, I2C0, embassy_rp::i2c::Async>) {
    log::info!("# I2C scan on I2C0:");
    let mut addr: u8 = 0x08;
    while addr < 0x78 {
        let mut dummy = [0u8; 1];
        if i2c.read_async(addr, &mut dummy).await.is_ok() {
            let mut s: String<32> = String::new();
            let _ = write!(s, "#   Found device at 0x{:02X}", addr);
            log::info!("{}", s.as_str());
        }
        addr += 1;
    }
    log::info!("# I2C scan complete");
}

// ============================================================================
// MAIN
// ============================================================================

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // ── USB logger (runs as a background task) ────────────────────────────────
    // All log::info!() calls are routed through this over USB CDC-ACM.
    // Open the Pico's USB serial port in any terminal to see output.
    spawner.spawn(logger_task(Driver::new(p.USB, Irqs)).unwrap());

    // Wait for the host to enumerate the USB device (~2 s is enough)
    Timer::after_millis(2000).await;

    // ── I2C0 ─────────────────────────────────────────────────────────────────
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 400_000;
    let mut i2c = I2c::new_async(p.I2C0, p.PIN_13, p.PIN_12, Irqs, i2c_config);

    // ── PWM (GPIO28, slice 6) ─────────────────────────────────────────────────
    let mut pwm_config = PwmConfig::default();
    pwm_config.top      = WRAP_CYCLE_COUNT;
    pwm_config.divider  = FixedU16::<U4>::from_num(38.15_f32);
    pwm_config.compare_a = 0;
    pwm_config.enable   = true;
    let pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone());

    // ── Enable pin (GPIO0) ────────────────────────────────────────────────────
    let enable_pin = Output::new(p.PIN_0, Level::Low);

    // ── BLiMS init ────────────────────────────────────────────────────────────
    let mut blims = Blims::new(Hardware { pwm, pwm_config, enable_pin });
    // Car test — both phases point to same target for simplicity
    blims.set_upwind_target(TARGET_LAT, TARGET_LON);
    blims.set_downwind_target(TARGET_LAT, TARGET_LON);
    blims.set_wind_from_deg(WIND_FROM_DEG);
    blims.set_wind_profile(&WIND_ALTITUDES_M, &WIND_DIRS_DEG);

    // Give the motor time to settle at neutral before starting
    Timer::after(Duration::from_secs(5)).await;

    // ── GPS init ──────────────────────────────────────────────────────────────
    if !gps_set_rate_20hz(&mut i2c).await {
        log::info!("# ERROR: GPS rate config failed");
    } else {
        log::info!("# GPS configured at 20 Hz");
    }

    i2c_scan(&mut i2c).await;

    // ── Banner ────────────────────────────────────────────────────────────────
    log::info!("# ================================================");
    log::info!("# BLiMS Car Test (FSW begin/execute pattern)");
    log::info!("# ================================================");
    {
        let mut s: String<64> = String::new();
        let _ = write!(s, "# Target:   {:.6}, {:.6}", TARGET_LAT, TARGET_LON);
        log::info!("{}", s.as_str());
    }
    {
        let mut s: String<64> = String::new();
        let _ = write!(s, "# Wind:     {} layers, surface {} deg",
            WIND_PROFILE_SIZE, WIND_DIRS_DEG[0] as u32);
        log::info!("{}", s.as_str());
    }
    //{
        //let mut s: String<80> = String::new();
        //let _ = write!(s, "# Descent:  {} samples, {:.1} -> {:.1} ft",
            //DESCENT_DATA_SIZE,
            //DESCENT_ALT_FT[0],
            //DESCENT_ALT_FT[DESCENT_DATA_SIZE - 1]);
        //log::info!("{}", s.as_str());

    //}
    log::info!("# Altitude: fixed 500 ft (tracking only, no descent)");

    {
        let mut s: String<48> = String::new();
        let _ = write!(s, "# Cycle:    {} ms (20 Hz)", CYCLE_TIME_MS);
        log::info!("{}", s.as_str());
    }
    {
        let mut s: String<64> = String::new();
        let _ = write!(s, "# Pins:     PWM={} EN={} SDA={} SCL={}",
            PWM_PIN_NUM, ENABLE_PIN_NUM, I2C_SDA_NUM, I2C_SCL_NUM);
        log::info!("{}", s.as_str());
    }
    log::info!("# CSV: lat,lon,target_lat,target_lon,heading,bearing,motor_pos,timestamp_ms,P,I,phase,altitude");
    log::info!("# ================================================");
    log::info!("# Waiting for GPS fix...");

    // ── Loop state ────────────────────────────────────────────────────────────
    let mut pvt             = UbxNavPvt::default();
    //let mut alt_index       = 0usize;
    //let mut descent_started = false;
    let mut last_phase_id: i8 = -1;

    // =========================================================================
    // MAIN LOOP — FSW-style 20 Hz cycle
    // =========================================================================
    loop {
        let cycle_start = Instant::now();

        // ── 1. Read GPS ───────────────────────────────────────────────────────
        if let Some(new_pvt) = read_nav_pvt(&mut i2c).await {
            pvt = new_pvt;
        }

        let gps_valid = pvt.fix_type >= 2;

        // ── 2. Altitude simulation ────────────────────────────────────────────
        //if !descent_started && gps_valid {
            //descent_started = true;
            //let mut s: String<80> = String::new();
            //let _ = write!(s, "# DESCENT STARTED — alt {:.1} ft (0/{})",
            //    DESCENT_ALT_FT[0], DESCENT_DATA_SIZE);
            //log::info!("{}", s.as_str());
        //}

        let current_alt_ft = 500.0;//DESCENT_ALT_FT[alt_index];

        // ── 3. Pack BlimsDataIn ───────────────────────────────────────────────
        let data_in = BlimsDataIn {
            lon:         pvt.lon,
            lat:         pvt.lat,
            altitude_ft: current_alt_ft, //if descent_started { current_alt_ft } else { DESCENT_ALT_FT[0] },
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

        // ── 5. Advance altitude ───────────────────────────────────────────────
        //if descent_started && alt_index < DESCENT_DATA_SIZE - 1 {
        //    alt_index += 1;
        //}

        // ── 6. Log ────────────────────────────────────────────────────────────

        // Phase-change banner
        if data_out.phase_id != last_phase_id {
            let mut s: String<80> = String::new();
            let _ = write!(s, "# PHASE: {} (alt={:.1} ft)",
                phase_name(data_out.phase_id),
                current_alt_ft);
                //alt_index,
                //DESCENT_DATA_SIZE);
            log::info!("{}", s.as_str());
            last_phase_id = data_out.phase_id;
        }

        // CSV row — only when GPS has a valid fix
        if pvt.fix_type >= 2 {
            let heading_deg = pvt.head_mot as f32 * 1e-5;
            let lat_f       = pvt.lat as f32 * 1e-7;
            let lon_f       = pvt.lon as f32 * 1e-7;
            let now_ms      = Instant::now().as_millis();

            // 12 fields — MVP CSV format:
            //   lat,lon,target_lat,target_lon,heading,bearing,motor_pos,
            //   timestamp_ms,P,I,phase,altitude
            let mut row: String<192> = String::new();
            let _ = write!(
                row,
                "{:.7},{:.7},{:.6},{:.6},{:.5},{:.5},{:.4},{},{:.6},{:.6},{},{:.2}",
                lat_f,
                lon_f,
                TARGET_LAT,
                TARGET_LON,
                heading_deg,
                data_out.bearing,
                data_out.brakeline_diff_in,
                now_ms,
                data_out.pid_p,
                data_out.pid_i,
                data_out.phase_id,
                current_alt_ft,
            );
            log::info!("{}", row.as_str());
        } else {
            let mut s: String<32> = String::new();
            let _ = write!(s, "# No fix (type={})", pvt.fix_type);
            log::info!("{}", s.as_str());
        }

        // ── 7. Cycle timing ───────────────────────────────────────────────────
        let elapsed_ms = cycle_start.elapsed().as_millis();
        if elapsed_ms < CYCLE_TIME_MS {
            Timer::after(Duration::from_millis(CYCLE_TIME_MS - elapsed_ms)).await;
        } else {
            let mut s: String<48> = String::new();
            let _ = write!(s, "# WARN: cycle overrun {} ms", elapsed_ms);
            log::info!("{}", s.as_str());
        }
    }
}
