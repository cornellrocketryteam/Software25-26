/// Dual ADS1015 ADC Monitor
///
/// Continuously reads all 4 channels from two ADS1015 ADCs
/// connected to I2C bus 2 at addresses 0x48 and 0x49.
///

use anyhow::Result;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(any(target_os = "linux", target_os = "android"))]
use fill_station::components::ads1015::{Ads1015, Channel, Gain, DataRate};

#[cfg(any(target_os = "linux", target_os = "android"))]
const I2C_BUS: &str = "/dev/i2c-2";
#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC1_ADDR: u16 = 0x48;
#[cfg(any(target_os = "linux", target_os = "android"))]
const ADC2_ADDR: u16 = 0x49;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn main() -> Result<()> {
    println!("============================================");
    println!("  Dual ADS1015 ADC Monitor");
    println!("Ronit & Max");
    println!("============================================\n");
    
    println!("Initializing I2C devices on {}...", I2C_BUS);
    
    // Initialize both ADCs
    let mut adc1 = Ads1015::new(I2C_BUS, ADC1_ADDR)?;
    let mut adc2 = Ads1015::new(I2C_BUS, ADC2_ADDR)?;
    
    println!("ADC1 ready at address 0x{:02X}", adc1.address());
    println!("ADC2 ready at address 0x{:02X}\n", adc2.address());
    
    println!("Configuration:");
    println!("  • Gain: ±6.144V range (2/3x)");
    println!("  • Sample Rate: 3300 SPS per channel");
    println!("  • Channels: 4 single-ended inputs per ADC (8 total)");
    println!("  • Mode: Continuous single-shot conversion\n");
    
    println!("Press Ctrl+C to stop\n");
    
    // Allow devices to settle
    thread::sleep(Duration::from_millis(200));
    
    // Channel configuration
    let channels = [Channel::Ain0, Channel::Ain1, Channel::Ain2, Channel::Ain3];
    let gain = Gain::TwoThirds; // ±6.144V range
    let data_rate = DataRate::Sps3300; // Maximum speed
    
    let mut loop_count = 0u64;
    let start_time = Instant::now();
    let mut last_print = Instant::now();
    
    loop {
        loop_count += 1;
        
        // Print header every 2 seconds
        if last_print.elapsed() >= Duration::from_secs(2) {
            print_header();
            last_print = Instant::now();
        }
        
        // Read all 8 channels
        let mut readings: [(i16, f32); 8] = [(0, 0.0); 8];
        
        // ADC1 channels (0-3)
        for (i, &channel) in channels.iter().enumerate() {
            let raw = adc1.read_raw(channel, gain, data_rate)?;
            let voltage = (raw as f32) * gain.lsb_size();
            readings[i] = (raw, voltage);
        }
        
        // ADC2 channels (4-7)
        for (i, &channel) in channels.iter().enumerate() {
            let raw = adc2.read_raw(channel, gain, data_rate)?;
            let voltage = (raw as f32) * gain.lsb_size();
            readings[i + 4] = (raw, voltage);
        }
        
        // Calculate throughput
        let elapsed = start_time.elapsed().as_secs_f32();
        let total_samples = loop_count * 8; // 8 channels per loop
        let samples_per_sec = if elapsed > 0.0 {
            (total_samples as f32) / elapsed
        } else {
            0.0
        };
        
        // Print readings in a compact format
        print!("\r{:6} │ {:7.1} SPS │ ", loop_count, samples_per_sec);
        
        // ADC1 readings
        for i in 0..4 {
            print!("{:>6} {:>6.3}V │ ", readings[i].0, readings[i].1);
        }
        
        print!("│ ");
        
        // ADC2 readings  
        for i in 4..8 {
            print!("{:>6} {:>6.3}V │ ", readings[i].0, readings[i].1);
        }
        
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        // Small delay to prevent overwhelming the terminal
        // Adjust or remove for maximum sampling rate
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_header() {
    println!("\n{:═<140}", "");
    println!("{:^6} │ {:^11} │ {:^64} │ {:^64}", 
             "Sample", "Throughput", 
             "ADC1 (0x48) - Ch0, Ch1, Ch2, Ch3",
             "ADC2 (0x49) - Ch0, Ch1, Ch2, Ch3");
    println!("{:^6} │ {:^11} │ {:^15} │ {:^15} │ {:^15} │ {:^15} │ {:^15} │ {:^15} │ {:^15} │ {:^15}", 
             "", "",
             "Raw / Volts", "Raw / Volts", "Raw / Volts", "Raw / Volts",
             "Raw / Volts", "Raw / Volts", "Raw / Volts", "Raw / Volts");
    println!("{:─<140}", "");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn main() {
    eprintln!("ERROR: This program requires Linux with I2C support.");
    eprintln!("It is designed to run on the TI AM64x SK board.");
    std::process::exit(1);
}
