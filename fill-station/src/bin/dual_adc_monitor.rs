/// Example program demonstrating dual ADS1015 ADC usage
///
/// This program reads from two ADS1015 ADCs connected on I2C bus 2
/// at addresses 0x48 and 0x49, continuously sampling all 4 channels
/// from each ADC and displaying the results.

use anyhow::Result;
use fill_station::components::ads1015::{Ads1015, Channel, Gain, DataRate};
use std::thread;
use std::time::{Duration, Instant};

const I2C_BUS: &str = "/dev/i2c-2";
const ADC1_ADDR: u16 = 0x48;
const ADC2_ADDR: u16 = 0x49;

fn main() -> Result<()> {
    println!("=== Fill Station Dual ADS1015 ADC Monitor ===");
    println!("Initializing ADCs on {}...\n", I2C_BUS);
    
    // Initialize both ADCs
    let mut adc1 = Ads1015::new(I2C_BUS, ADC1_ADDR)?;
    let mut adc2 = Ads1015::new(I2C_BUS, ADC2_ADDR)?;
    
    println!("✓ ADC1 initialized at address 0x{:02X}", adc1.address());
    println!("✓ ADC2 initialized at address 0x{:02X}", adc2.address());
    
    println!("\nConfiguration:");
    println!("  Gain: ±6.144V range (2/3x gain)");
    println!("  Data Rate: 3300 SPS (maximum)");
    println!("  Mode: Single-shot conversion");
    println!("  Channels: Reading all 4 single-ended inputs per ADC");
    println!("\nPress Ctrl+C to stop\n");
    
    // Small delay to let things settle
    thread::sleep(Duration::from_millis(100));
    
    let channels = [Channel::Ain0, Channel::Ain1, Channel::Ain2, Channel::Ain3];
    let gain = Gain::TwoThirds; // ±6.144V range
    let data_rate = DataRate::Sps3300; // Maximum speed
    
    let mut sample_count = 0;
    let start_time = Instant::now();
    
    loop {
        sample_count += 1;
        
        // Print header every 50 readings for readability
        if sample_count % 50 == 1 {
            println!("\n{:-<140}", "");
            println!("{:>6} | {:>8} | ADC1 (0x{:02X}) - Raw Values & Voltages          | ADC2 (0x{:02X}) - Raw Values & Voltages", 
                     "Sample", "Rate", ADC1_ADDR, ADC2_ADDR);
            println!("{:>6} | {:>8} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7} | {:>6} {:>7}", 
                     "", "(SPS)",
                     "Ch0", "V", "Ch1", "V", "Ch2", "V", "Ch3", "V",
                     "Ch0", "V", "Ch1", "V", "Ch2", "V", "Ch3", "V");
            println!("{:-<140}", "");
        }
        
        // Read all channels from both ADCs
        let mut adc1_raw = [0i16; 4];
        let mut adc1_volts = [0.0f32; 4];
        let mut adc2_raw = [0i16; 4];
        let mut adc2_volts = [0.0f32; 4];
        
        for (i, &channel) in channels.iter().enumerate() {
            adc1_raw[i] = adc1.read_raw(channel, gain, data_rate)?;
            adc1_volts[i] = adc1_raw[i] as f32 * gain.lsb_size();
            
            adc2_raw[i] = adc2.read_raw(channel, gain, data_rate)?;
            adc2_volts[i] = adc2_raw[i] as f32 * gain.lsb_size();
        }
        
        // Calculate actual samples per second (8 total channels)
        let elapsed = start_time.elapsed().as_secs_f32();
        let sps = if elapsed > 0.0 {
            (sample_count as f32 * 8.0) / elapsed
        } else {
            0.0
        };
        
        // Print the readings
        println!("{:>6} | {:>8.1} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3} | {:>6} {:>7.3}",
                 sample_count, sps,
                 adc1_raw[0], adc1_volts[0], 
                 adc1_raw[1], adc1_volts[1], 
                 adc1_raw[2], adc1_volts[2], 
                 adc1_raw[3], adc1_volts[3],
                 adc2_raw[0], adc2_volts[0], 
                 adc2_raw[1], adc2_volts[1], 
                 adc2_raw[2], adc2_volts[2], 
                 adc2_raw[3], adc2_volts[3]);
        
        // Small delay to prevent terminal overflow
        // Remove or adjust for maximum sampling speed
        thread::sleep(Duration::from_millis(50));
    }
}
