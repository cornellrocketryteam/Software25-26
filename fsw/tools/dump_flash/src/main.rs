//! dump_flash — Cornell Rocketry Team
//! =====================================
//! Host-side tool that sends the DumpFlash command (`<G>`) over the USB
//! umbilical serial port, captures the binary dump from onboard flash, decodes
//! it into CSV, and saves it to a timestamped `.csv` file.
//!
//! Within FSW directory:
//!
//!     tools\dump_flash.bat
//!
//! Within dump_flash directory:
//!   cargo run                     # auto-detect COM port
//!   cargo run -- COM4             # explicit port (Windows)
//!   cargo run -- /dev/ttyACM0     # explicit port (Linux/Mac)
//!   cargo run -- COM4 115200      # explicit port + baud
//!
//! Note: close any serial monitor on the same port before running.

use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};

use chrono::Local;
use serialport::SerialPort;

// ── Serial constants ──────────────────────────────────────────────────────────

const DEFAULT_BAUD: u32        = 2000000;
const DUMP_CMD: &[u8]          = b"<G>";
const HEARTBEAT_CMD: &[u8]     = b"<H>";
const HEARTBEAT_INTERVAL_MS: u64 = 1000; // FSW disconnects after 3 s without <H>
const DUMP_TIMEOUT_S: u64      = 7200;   // 2 hr hard cap
const SILENCE_TIMEOUT_MS: u64  = 3000;  // give up after this many ms of silence
const PORT_READ_TIMEOUT_MS: u64 = 100;
const READ_BUF_SIZE: usize     = 4096;

// These strings appear as plain-text lines framing the raw binary payload.
const BEGIN_MARKER: &[u8] = b"BEGIN FLASH BINARY DUMP";
const END_MARKER: &[u8]   = b"END FLASH BINARY DUMP";

// ── Binary record layout ──────────────────────────────────────────────────────
// Each record on flash: [tag: u8] [payload: N bytes], all little-endian.

const FAST_TAG: u8  = 0xFA;
const FULL_TAG: u8  = 0xFB;
const FAST_SIZE: usize = 84;  // payload bytes (tag not included)
const FULL_SIZE: usize = 199; // payload bytes (tag not included)

// Fast record payload offsets
mod fast {
    pub const MS_SINCE_BOOT:       usize = 0;   // u32
    pub const FLIGHT_MODE:         usize = 4;   // u32
    pub const PRESSURE:            usize = 8;   // f32
    pub const TEMP:                usize = 12;  // f32
    pub const ALTITUDE:            usize = 16;  // f32
    pub const MAG_X:               usize = 20;  // f32
    pub const MAG_Y:               usize = 24;  // f32
    pub const MAG_Z:               usize = 28;  // f32
    pub const ACCEL_X:             usize = 32;  // f32
    pub const ACCEL_Y:             usize = 36;  // f32
    pub const ACCEL_Z:             usize = 40;  // f32
    pub const GYRO_X:              usize = 44;  // f32
    pub const GYRO_Y:              usize = 48;  // f32
    pub const GYRO_Z:              usize = 52;  // f32
    pub const PT3:                 usize = 56;  // f32
    pub const PT4:                 usize = 60;  // f32
    pub const RTD:                 usize = 64;  // f32
    pub const SV_OPEN:             usize = 68;  // u8
    pub const MAV_OPEN:            usize = 69;  // u8
    pub const SSA_DROGUE:          usize = 70;  // u8
    pub const SSA_MAIN:            usize = 71;  // u8
    pub const CMD_N1:              usize = 72;  // u8
    pub const CMD_N2:              usize = 73;  // u8
    pub const CMD_N3:              usize = 74;  // u8
    pub const CMD_N4:              usize = 75;  // u8
    pub const CMD_A1:              usize = 76;  // u8
    pub const CMD_A2:              usize = 77;  // u8
    pub const CMD_A3:              usize = 78;  // u8
    pub const AIRBRAKE_STATE:      usize = 79;  // u8
    pub const PREDICTED_APOGEE:    usize = 80;  // f32
}

// Full record payload offsets (mirrors Packet::to_bytes() in packet.rs)
mod full {
    pub const FLIGHT_MODE:         usize = 0;   // u32
    pub const PRESSURE:            usize = 4;   // f32
    pub const TEMP:                usize = 8;   // f32
    pub const ALTITUDE:            usize = 12;  // f32
    pub const LATITUDE:            usize = 16;  // f32
    pub const LONGITUDE:           usize = 20;  // f32
    pub const NUM_SATELLITES:      usize = 24;  // u32
    pub const TIMESTAMP:           usize = 28;  // f32
    pub const MAG_X:               usize = 32;  // f32
    pub const MAG_Y:               usize = 36;  // f32
    pub const MAG_Z:               usize = 40;  // f32
    pub const ACCEL_X:             usize = 44;  // f32
    pub const ACCEL_Y:             usize = 48;  // f32
    pub const ACCEL_Z:             usize = 52;  // f32
    pub const GYRO_X:              usize = 56;  // f32
    pub const GYRO_Y:              usize = 60;  // f32
    pub const GYRO_Z:              usize = 64;  // f32
    pub const PT3:                 usize = 68;  // f32
    pub const PT4:                 usize = 72;  // f32
    pub const RTD:                 usize = 76;  // f32
    pub const SV_OPEN:             usize = 80;  // u8
    pub const MAV_OPEN:            usize = 81;  // u8
    pub const SSA_DROGUE:          usize = 82;  // u8
    pub const SSA_MAIN:            usize = 83;  // u8
    pub const CMD_N1:              usize = 84;  // u8
    pub const CMD_N2:              usize = 85;  // u8
    pub const CMD_N3:              usize = 86;  // u8
    pub const CMD_N4:              usize = 87;  // u8
    pub const CMD_A1:              usize = 88;  // u8
    pub const CMD_A2:              usize = 89;  // u8
    pub const CMD_A3:              usize = 90;  // u8
    pub const AIRBRAKE_STATE:      usize = 91;  // u8
    pub const PREDICTED_APOGEE:    usize = 92;  // f32
    pub const H_ACC:               usize = 96;  // u32
    pub const V_ACC:               usize = 100; // u32
    pub const VEL_N:               usize = 104; // f64
    pub const VEL_E:               usize = 112; // f64
    pub const VEL_D:               usize = 120; // f64
    pub const G_SPEED:             usize = 128; // f64
    pub const S_ACC:               usize = 136; // u32
    pub const HEAD_ACC:            usize = 140; // u32
    pub const FIX_TYPE:            usize = 144; // u8
    pub const HEAD_MOT:            usize = 145; // i32
    pub const BLIMS_MOTOR_POS:     usize = 149; // f32
    pub const BLIMS_PHASE_ID:      usize = 153; // i8
    pub const BLIMS_PID_P:         usize = 154; // f32
    pub const BLIMS_PID_I:         usize = 158; // f32
    pub const BLIMS_BEARING:       usize = 162; // f32
    pub const BLIMS_LOITER_STEP:   usize = 166; // i8
    pub const BLIMS_HEADING_DES:   usize = 167; // f32
    pub const BLIMS_HEADING_ERR:   usize = 171; // f32
    pub const BLIMS_ERR_INTEGRAL:  usize = 175; // f32
    pub const BLIMS_DIST_TO_TGT:   usize = 179; // f32
    pub const BLIMS_TARGET_LAT:    usize = 183; // f32
    pub const BLIMS_TARGET_LON:    usize = 187; // f32
    pub const BLIMS_WIND_FROM_DEG: usize = 191; // f32
    pub const MS_SINCE_BOOT:       usize = 195; // u32
}

// ── Carry-forward state for GPS / BLiMS (absent in fast records) ─────────────

#[derive(Default, Clone)]
struct SlowFields {
    latitude:            f32,
    longitude:           f32,
    num_satellites:      u32,
    timestamp:           f32,
    h_acc:               u32,
    v_acc:               u32,
    vel_n:               f64,
    vel_e:               f64,
    vel_d:               f64,
    g_speed:             f64,
    s_acc:               u32,
    head_acc:            u32,
    fix_type:            u8,
    head_mot:            i32,
    blims_motor_pos:     f32,
    blims_phase_id:      i8,
    blims_pid_p:         f32,
    blims_pid_i:         f32,
    blims_bearing:       f32,
    blims_loiter_step:   i8,
    blims_heading_des:   f32,
    blims_heading_err:   f32,
    blims_err_integral:  f32,
    blims_dist_to_tgt:   f32,
    blims_target_lat:    f32,
    blims_target_lon:    f32,
    blims_wind_from_deg: f32,
}

// ── Decode helpers ────────────────────────────────────────────────────────────

fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off+4].try_into().unwrap())
}
fn i32le(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off+4].try_into().unwrap())
}
fn f32le(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off+4].try_into().unwrap())
}
fn f64le(b: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(b[off..off+8].try_into().unwrap())
}

/// Emit one CSV data row from a full-record payload (199 bytes).
fn csv_from_full(p: &[u8], slow: &mut SlowFields) -> String {
    slow.latitude            = f32le(p, full::LATITUDE);
    slow.longitude           = f32le(p, full::LONGITUDE);
    slow.num_satellites      = u32le(p, full::NUM_SATELLITES);
    slow.timestamp           = f32le(p, full::TIMESTAMP);
    slow.h_acc               = u32le(p, full::H_ACC);
    slow.v_acc               = u32le(p, full::V_ACC);
    slow.vel_n               = f64le(p, full::VEL_N);
    slow.vel_e               = f64le(p, full::VEL_E);
    slow.vel_d               = f64le(p, full::VEL_D);
    slow.g_speed             = f64le(p, full::G_SPEED);
    slow.s_acc               = u32le(p, full::S_ACC);
    slow.head_acc            = u32le(p, full::HEAD_ACC);
    slow.fix_type            = p[full::FIX_TYPE];
    slow.head_mot            = i32le(p, full::HEAD_MOT);
    slow.blims_motor_pos     = f32le(p, full::BLIMS_MOTOR_POS);
    slow.blims_phase_id      = p[full::BLIMS_PHASE_ID] as i8;
    slow.blims_pid_p         = f32le(p, full::BLIMS_PID_P);
    slow.blims_pid_i         = f32le(p, full::BLIMS_PID_I);
    slow.blims_bearing       = f32le(p, full::BLIMS_BEARING);
    slow.blims_loiter_step   = p[full::BLIMS_LOITER_STEP] as i8;
    slow.blims_heading_des   = f32le(p, full::BLIMS_HEADING_DES);
    slow.blims_heading_err   = f32le(p, full::BLIMS_HEADING_ERR);
    slow.blims_err_integral  = f32le(p, full::BLIMS_ERR_INTEGRAL);
    slow.blims_dist_to_tgt   = f32le(p, full::BLIMS_DIST_TO_TGT);
    slow.blims_target_lat    = f32le(p, full::BLIMS_TARGET_LAT);
    slow.blims_target_lon    = f32le(p, full::BLIMS_TARGET_LON);
    slow.blims_wind_from_deg = f32le(p, full::BLIMS_WIND_FROM_DEG);

    format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        u32le(p, full::FLIGHT_MODE),
        f32le(p, full::PRESSURE),
        f32le(p, full::TEMP),
        f32le(p, full::ALTITUDE),
        slow.latitude,
        slow.longitude,
        slow.num_satellites,
        slow.timestamp,
        f32le(p, full::MAG_X),
        f32le(p, full::MAG_Y),
        f32le(p, full::MAG_Z),
        f32le(p, full::ACCEL_X),
        f32le(p, full::ACCEL_Y),
        f32le(p, full::ACCEL_Z),
        f32le(p, full::GYRO_X),
        f32le(p, full::GYRO_Y),
        f32le(p, full::GYRO_Z),
        f32le(p, full::PT3),
        f32le(p, full::PT4),
        f32le(p, full::RTD),
        p[full::SV_OPEN],
        p[full::MAV_OPEN],
        p[full::SSA_DROGUE],
        p[full::SSA_MAIN],
        p[full::CMD_N1],
        p[full::CMD_N2],
        p[full::CMD_N3],
        p[full::CMD_N4],
        p[full::CMD_A1],
        p[full::CMD_A2],
        p[full::CMD_A3],
        p[full::AIRBRAKE_STATE],
        f32le(p, full::PREDICTED_APOGEE),
        slow.h_acc,
        slow.v_acc,
        slow.vel_n,
        slow.vel_e,
        slow.vel_d,
        slow.g_speed,
        slow.s_acc,
        slow.head_acc,
        slow.fix_type,
        slow.head_mot,
        slow.blims_motor_pos,
        slow.blims_phase_id,
        slow.blims_pid_p,
        slow.blims_pid_i,
        slow.blims_bearing,
        slow.blims_loiter_step,
        slow.blims_heading_des,
        slow.blims_heading_err,
        slow.blims_err_integral,
        slow.blims_dist_to_tgt,
        slow.blims_target_lat,
        slow.blims_target_lon,
        slow.blims_wind_from_deg,
        u32le(p, full::MS_SINCE_BOOT),
    )
}

/// Emit one CSV data row from a fast-record payload (84 bytes), filling
/// GPS / BLiMS columns from carry-forward `slow`.
fn csv_from_fast(p: &[u8], slow: &SlowFields) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        u32le(p, fast::FLIGHT_MODE),
        f32le(p, fast::PRESSURE),
        f32le(p, fast::TEMP),
        f32le(p, fast::ALTITUDE),
        slow.latitude,
        slow.longitude,
        slow.num_satellites,
        slow.timestamp,
        f32le(p, fast::MAG_X),
        f32le(p, fast::MAG_Y),
        f32le(p, fast::MAG_Z),
        f32le(p, fast::ACCEL_X),
        f32le(p, fast::ACCEL_Y),
        f32le(p, fast::ACCEL_Z),
        f32le(p, fast::GYRO_X),
        f32le(p, fast::GYRO_Y),
        f32le(p, fast::GYRO_Z),
        f32le(p, fast::PT3),
        f32le(p, fast::PT4),
        f32le(p, fast::RTD),
        p[fast::SV_OPEN],
        p[fast::MAV_OPEN],
        p[fast::SSA_DROGUE],
        p[fast::SSA_MAIN],
        p[fast::CMD_N1],
        p[fast::CMD_N2],
        p[fast::CMD_N3],
        p[fast::CMD_N4],
        p[fast::CMD_A1],
        p[fast::CMD_A2],
        p[fast::CMD_A3],
        p[fast::AIRBRAKE_STATE],
        f32le(p, fast::PREDICTED_APOGEE),
        slow.h_acc,
        slow.v_acc,
        slow.vel_n,
        slow.vel_e,
        slow.vel_d,
        slow.g_speed,
        slow.s_acc,
        slow.head_acc,
        slow.fix_type,
        slow.head_mot,
        slow.blims_motor_pos,
        slow.blims_phase_id,
        slow.blims_pid_p,
        slow.blims_pid_i,
        slow.blims_bearing,
        slow.blims_loiter_step,
        slow.blims_heading_des,
        slow.blims_heading_err,
        slow.blims_err_integral,
        slow.blims_dist_to_tgt,
        slow.blims_target_lat,
        slow.blims_target_lon,
        slow.blims_wind_from_deg,
        u32le(p, fast::MS_SINCE_BOOT),
    )
}

/// Walk the raw binary buffer and decode all records into CSV rows.
/// Stops at the first run of 0xFF bytes (erased flash) or end of buffer.
/// Returns (fast_count, full_count, skipped_bytes).
fn decode_binary(buf: &[u8], csv_rows: &mut Vec<String>) -> (usize, usize, usize) {
    let mut slow = SlowFields::default();
    let mut saw_full = false;
    let mut fast_count = 0usize;
    let mut full_count = 0usize;
    let mut skipped   = 0usize;
    let mut i = 0usize;

    while i < buf.len() {
        // Erased flash sentinel — data ends here.
        if buf[i] == 0xFF {
            break;
        }

        match buf[i] {
            FAST_TAG => {
                let end = i + 1 + FAST_SIZE;
                if end > buf.len() { break; }
                let payload = &buf[i+1..end];
                if !saw_full {
                    // GPS/BLiMS columns will be 0 — warn once at end.
                }
                csv_rows.push(csv_from_fast(payload, &slow));
                fast_count += 1;
                i = end;
            }
            FULL_TAG => {
                let end = i + 1 + FULL_SIZE;
                if end > buf.len() { break; }
                let payload = &buf[i+1..end];
                csv_rows.push(csv_from_full(payload, &mut slow));
                full_count += 1;
                saw_full = true;
                i = end;
            }
            _ => {
                // Unknown byte — skip forward one byte and keep scanning.
                skipped += 1;
                i += 1;
            }
        }
    }

    if !saw_full && fast_count > 0 {
        eprintln!(
            "WARNING: No full records found — GPS/BLiMS columns will be all zeros \
             ({} fast records decoded).",
            fast_count
        );
    }

    (fast_count, full_count, skipped)
}

// ── Port auto-detection ───────────────────────────────────────────────────────

fn find_port() -> String {
    let ports = serialport::available_ports().unwrap_or_default();

    for port in &ports {
        let name = port.port_name.to_lowercase();
        if let serialport::SerialPortType::UsbPort(ref info) = port.port_type {
            let mfg  = info.manufacturer.as_deref().unwrap_or("").to_lowercase();
            let prod = info.product.as_deref().unwrap_or("").to_lowercase();
            if mfg.contains("raspberry")
                || mfg.contains("tinyusb")
                || prod.contains("pico")
                || prod.contains("rp2")
                || prod.contains("crt")
                || name.contains("crt")
            {
                println!("Auto-detected port: {} ({})", port.port_name, prod);
                return port.port_name.clone();
            }
        }
    }

    if ports.len() == 1 {
        println!("Auto-detected (only available port): {}", ports[0].port_name);
        return ports[0].port_name.clone();
    }

    eprintln!("Available ports:");
    for p in &ports {
        eprintln!("  {}", p.port_name);
    }
    eprintln!("\nCould not auto-detect port. Pass it as an argument:");
    eprintln!("  cargo run -- COM4");
    std::process::exit(1);
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let port_name = if args.len() > 1 { args[1].clone() } else { find_port() };
    let baud      = if args.len() > 2 { args[2].parse().unwrap_or(DEFAULT_BAUD) } else { DEFAULT_BAUD };

    println!("Opening {} @ {} baud...", port_name, baud);

    let mut port: Box<dyn SerialPort> = serialport::new(&port_name, baud)
        .timeout(Duration::from_millis(PORT_READ_TIMEOUT_MS))
        .open()
        .unwrap_or_else(|e| {
            eprintln!("ERROR: Could not open {}: {}", port_name, e);
            std::process::exit(1);
        });

    std::thread::sleep(Duration::from_millis(1000));
    let _ = port.clear(serialport::ClearBuffer::All);

    println!("Sending DumpFlash command: {} ...", String::from_utf8_lossy(DUMP_CMD));
    port.write_all(DUMP_CMD).expect("Failed to write to serial port");
    port.flush().expect("Failed to flush serial port");

    // ── Reception ─────────────────────────────────────────────────────────────
    // Phase 1: line-by-line until BEGIN marker.
    // Phase 2: raw byte accumulation until END marker found in the stream.

    let mut dump_started  = false;
    let mut dump_done     = false;
    let mut line_buf      = Vec::<u8>::new();
    let mut binary_buf    = Vec::<u8>::new();
    let mut read_buf      = vec![0u8; READ_BUF_SIZE];

    let deadline          = Instant::now() + Duration::from_secs(DUMP_TIMEOUT_S);
    let mut last_rx       = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut total_bytes: u64 = 0;
    let start_time        = Instant::now();
    let mut last_progress = Instant::now();

    println!("Waiting for dump response...\n");

    'outer: loop {
        if Instant::now() >= deadline {
            println!("\nWARNING: Hard timeout ({} s) reached.", DUMP_TIMEOUT_S);
            break;
        }

        if dump_started && last_rx.elapsed() >= Duration::from_millis(SILENCE_TIMEOUT_MS) {
            println!("\nWARNING: No data for {} ms — assuming dump complete.", SILENCE_TIMEOUT_MS);
            break;
        }

        // Keep the umbilical alive — FSW drops connection after 3 s without <H>.
        if last_heartbeat.elapsed() >= Duration::from_millis(HEARTBEAT_INTERVAL_MS) {
            let _ = port.write_all(HEARTBEAT_CMD);
            let _ = port.flush();
            last_heartbeat = Instant::now();
        }

        match port.read(&mut read_buf) {
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(_) => {
                if dump_started { println!("\nWARNING: Serial read error — stopping."); }
                break;
            }
            Ok(0) => continue,
            Ok(n) => {
                last_rx = Instant::now();
                total_bytes += n as u64;

                if !dump_started {
                    // Phase 1: assemble text lines and look for the begin marker.
                    for &b in &read_buf[..n] {
                        if b == b'\n' {
                            if line_buf.windows(BEGIN_MARKER.len()).any(|w| w == BEGIN_MARKER) {
                                dump_started = true;
                                line_buf.clear();
                                println!("  ✓ Binary dump started");
                                break;
                            }
                            line_buf.clear();
                        } else if b != b'\r' {
                            line_buf.push(b);
                        }
                    }
                } else {
                    // Phase 2: accumulate raw bytes and scan the tail for the end marker.
                    let chunk = &read_buf[..n];
                    binary_buf.extend_from_slice(chunk);

                    // Check if the end marker appears anywhere in the newly extended tail.
                    // We only need to search the last (END_MARKER.len() + n) bytes for efficiency.
                    let search_start = binary_buf
                        .len()
                        .saturating_sub(END_MARKER.len() + n + 4);
                    if let Some(pos) = binary_buf[search_start..]
                        .windows(END_MARKER.len())
                        .position(|w| w == END_MARKER)
                    {
                        // Trim everything from the line containing the end marker.
                        // Find the '\n' (or start of the marker line) before pos.
                        let abs_pos = search_start + pos;
                        let trim_at = binary_buf[..abs_pos]
                            .iter()
                            .rposition(|&b| b == b'\n')
                            .map(|p| p + 1)
                            .unwrap_or(abs_pos);
                        binary_buf.truncate(trim_at);

                        let elapsed = start_time.elapsed().as_secs_f32();
                        let kb = total_bytes as f32 / 1024.0;
                        println!(
                            "\n  ✓ Dump complete — {:.1} KB in {:.1}s ({:.1} KB/s)",
                            kb, elapsed, kb / elapsed
                        );
                        dump_done = true;
                        break 'outer;
                    }

                    // Progress update every 2 seconds.
                    if last_progress.elapsed() >= Duration::from_secs(2) {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let kb = total_bytes as f32 / 1024.0;
                        print!("\r  {:.1} KB  |  {:.1} KB/s  |  {} binary bytes buffered   ",
                            kb, kb / elapsed, binary_buf.len());
                        let _ = std::io::stdout().flush();
                        last_progress = Instant::now();
                    }
                }
            }
        }
    }
    println!();

    if !dump_started {
        eprintln!("ERROR: Never received BEGIN FLASH BINARY DUMP marker.");
        eprintln!("  • Make sure the firmware is flashed in RELEASE mode (not debug).");
        eprintln!("  • Make sure no other serial monitor has the port open.");
        std::process::exit(1);
    }

    if binary_buf.is_empty() {
        println!("WARNING: Dump was empty — no data in flash.");
        return;
    }

    if !dump_done {
        println!("NOTE: END marker not received — decoding whatever was captured.");
    }

    // ── Decode binary → CSV ───────────────────────────────────────────────────

    println!("Decoding {} binary bytes...", binary_buf.len());
    let mut csv_rows: Vec<String> = Vec::new();
    let (fast_count, full_count, skipped) = decode_binary(&binary_buf, &mut csv_rows);
    println!(
        "  {} fast records + {} full records = {} total rows  ({} bytes skipped)",
        fast_count, full_count, csv_rows.len(), skipped
    );

    if csv_rows.is_empty() {
        println!("WARNING: No valid records decoded — flash may contain old CSV data.");
        println!("  Wipe the flash with <W> and re-flash firmware before logging.");
        return;
    }

    // ── Write CSV ─────────────────────────────────────────────────────────────

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename  = format!("fsw_{}.csv", timestamp);
    let out_path  = std::env::current_dir().unwrap_or_default().join(&filename);

    let file = File::create(&out_path).unwrap_or_else(|e| {
        eprintln!("ERROR: Could not create output file: {}", e);
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);

    // Header row (matches Packet::CSV_HEADER in packet.rs)
    writeln!(writer,
        "flight_mode,pressure,temp,altitude,latitude,longitude,num_satellites,timestamp,\
         mag_x,mag_y,mag_z,accel_x,accel_y,accel_z,gyro_x,gyro_y,gyro_z,\
         pt3,pt4,rtd,sv_open,mav_open,ssa_drogue_deployed,ssa_main_deployed,\
         cmd_n1,cmd_n2,cmd_n3,cmd_n4,cmd_a1,cmd_a2,cmd_a3,\
         airbrake_state,predicted_apogee,h_acc,v_acc,\
         vel_n,vel_e,vel_d,g_speed,s_acc,head_acc,fix_type,head_mot,\
         blims_motor_position,blims_phase_id,blims_pid_p,blims_pid_i,blims_bearing,\
         blims_loiter_step,blims_heading_des,blims_heading_error,blims_error_integral,\
         blims_dist_to_target_m,blims_target_lat,blims_target_lon,blims_wind_from_deg,\
         ms_since_boot_cfc"
    ).expect("Failed to write header");

    for row in &csv_rows {
        write!(writer, "{}", row).expect("Failed to write row");
    }
    writer.flush().expect("Failed to flush file");

    println!("Saved  →  {}", out_path.display());
    println!("         {} data rows", csv_rows.len());
}
