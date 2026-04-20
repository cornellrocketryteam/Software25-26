//! dump_flash — Cornell Rocketry Team
//! =====================================
//! Host-side tool that sends the DumpFlash command (`<G>`) over the USB
//! umbilical serial port, captures the full CSV response from the onboard
//! QSPI flash, and saves it to a timestamped `.csv` file.
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

// Constants
const DEFAULT_BAUD: u32      = 2000000;
const DUMP_CMD: &[u8]        = b"<G>";
const DUMP_TIMEOUT_S: u64    = 7200; // 2 hr hard cap — covers full 14 MB flash at USB CDC speeds
const SILENCE_TIMEOUT_MS: u64 = 3000; // give up if no bytes received for this long after dump starts
const PORT_READ_TIMEOUT_MS: u64 = 100; // short poll interval so we can check silence/deadline
const READ_BUF_SIZE: usize   = 4096;  // read in large chunks instead of 1 byte at a time
const BEGIN_MARKER: &str     = "BEGIN FLASH CSV DUMP";
const END_MARKER: &str       = "END FLASH CSV DUMP";

// Port auto-detection
fn find_port() -> String {
    let ports = serialport::available_ports().unwrap_or_default();

    // Look for RP2350 / TinyUSB signatures in the port description
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

    // Fallback: only one port available
    if ports.len() == 1 {
        println!("Auto-detected (only available port): {}", ports[0].port_name);
        return ports[0].port_name.clone();
    }

    // Give up and show the list
    eprintln!("Available ports:");
    for p in &ports {
        eprintln!("  {}", p.port_name);
    }
    eprintln!("\nCould not auto-detect port. Pass it as an argument:");
    eprintln!("  cargo run -- COM4");
    std::process::exit(1);
}

// Main
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

    // Brief settle time so the device recognises the connection
    std::thread::sleep(Duration::from_millis(1000));

    // Flush stale data
    let _ = port.clear(serialport::ClearBuffer::All);

    // Send dump command
    println!("Sending DumpFlash command: {} ...", String::from_utf8_lossy(DUMP_CMD));
    port.write_all(DUMP_CMD).expect("Failed to write to serial port");
    port.flush().expect("Failed to flush serial port");

    // Read response — chunked reads for speed, manual line assembly
    let mut raw_lines: Vec<String> = Vec::new();
    let mut csv_started = false;
    let mut line_buf = Vec::<u8>::new();
    let mut read_buf = vec![0u8; READ_BUF_SIZE];

    let deadline     = Instant::now() + Duration::from_secs(DUMP_TIMEOUT_S);
    let mut last_rx  = Instant::now();
    let mut total_bytes: u64 = 0;
    let start_time   = Instant::now();
    let mut last_progress = Instant::now();

    println!("Waiting for dump response...\n");

    'outer: loop {
        // Hard deadline
        if Instant::now() >= deadline {
            println!("\nWARNING: Hard timeout ({} s) reached before END marker.", DUMP_TIMEOUT_S);
            break;
        }

        // Silence timeout — only after dump has started
        if csv_started && last_rx.elapsed() >= Duration::from_millis(SILENCE_TIMEOUT_MS) {
            println!("\nWARNING: No data for {} ms — assuming dump complete.", SILENCE_TIMEOUT_MS);
            break;
        }

        match port.read(&mut read_buf) {
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // No data in this poll window — loop back to check timeouts
                continue;
            }
            Err(_) => {
                if csv_started {
                    println!("\nWARNING: Serial read error — stopping.");
                }
                break;
            }
            Ok(0) => continue,
            Ok(n) => {
                last_rx = Instant::now();
                total_bytes += n as u64;

                // Process each received byte
                for &b in &read_buf[..n] {
                    if b == b'\n' {
                        let text = String::from_utf8_lossy(&line_buf)
                            .trim_end_matches('\r')
                            .to_string();
                        line_buf.clear();

                        if text.is_empty() {
                            continue;
                        }
                        raw_lines.push(text.clone());

                        if text.contains(BEGIN_MARKER) {
                            csv_started = true;
                            println!("  ✓ Dump started");
                            continue;
                        }
                        if text.contains(END_MARKER) {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            let kb = total_bytes as f32 / 1024.0;
                            println!(
                                "\n  ✓ Dump complete — {} lines in {:.1}s ({:.1} KB/s)",
                                raw_lines.len(),
                                elapsed,
                                kb / elapsed
                            );
                            break 'outer;
                        }
                    } else {
                        line_buf.push(b);
                    }
                }

                // Progress update every 2 seconds
                if csv_started && last_progress.elapsed() >= Duration::from_secs(2) {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let kb = total_bytes as f32 / 1024.0;
                    print!("\r  {:.1} KB  |  {:.1} KB/s  |  {} lines   ",
                        kb, kb / elapsed, raw_lines.len());
                    let _ = std::io::stdout().flush();
                    last_progress = Instant::now();
                }
            }
        }
    }
    println!();

    if !csv_started {
        eprintln!("ERROR: Never received BEGIN FLASH CSV DUMP marker.");
        eprintln!("  • Make sure the firmware is flashed in RELEASE mode (not debug).");
        eprintln!("  • Make sure no other serial monitor has the port open.");
        std::process::exit(1);
    }

    // Filter to pure CSV (strip markers)
    let csv_lines: Vec<&str> = raw_lines
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.contains(BEGIN_MARKER) && !s.contains(END_MARKER))
        .collect();

    if csv_lines.is_empty() {
        println!("WARNING: Dump was empty — no data rows found in flash.");
        return;
    }

    // Save to file
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename  = format!("fsw_{}.csv", timestamp);

    let out_path = std::env::current_dir()
        .unwrap_or_default()
        .join(&filename);

    let file = File::create(&out_path).unwrap_or_else(|e| {
        eprintln!("ERROR: Could not create output file: {}", e);
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    for line in &csv_lines {
        writeln!(writer, "{}", line).expect("Failed to write line");
    }
    writer.flush().expect("Failed to flush file");

    // Summary
    let data_rows = csv_lines.iter().filter(|l| !l.contains("flight_mode")).count();
    println!("Saved  →  {}", out_path.display());
    println!("         {} data rows  |  {} total lines", data_rows, csv_lines.len());
    if data_rows == 0 {
        println!("NOTE: Flash contained only the CSV header — no packets were logged yet.");
    }
}
