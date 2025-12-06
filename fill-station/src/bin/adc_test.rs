/// ADS1015 I2C Test and Debug Tool
///
/// This tool helps diagnose issues with ADS1015 ADC communication.
/// It tests I2C connectivity, reads device registers, and shows detailed debug info.

use anyhow::Result;
use std::thread;
use std::time::Duration;

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
    println!("====================================");
    println!("  ADS1015 I2C Diagnostic Tool");
    println!("====================================\n");
    
    println!("Configuration:");
    println!("  I2C Bus: {}", I2C_BUS);
    println!("  ADC1 Address: 0x{:02X}", ADC1_ADDR);
    println!("  ADC2 Address: 0x{:02X}\n", ADC2_ADDR);
    
    // Test ADC1
    println!("Testing ADC1 (0x{:02X})...", ADC1_ADDR);
    match test_adc(I2C_BUS, ADC1_ADDR) {
        Ok(_) => println!("✓ ADC1 test passed\n"),
        Err(e) => println!("✗ ADC1 test failed: {}\n", e),
    }
    
    // Test ADC2
    println!("Testing ADC2 (0x{:02X})...", ADC2_ADDR);
    match test_adc(I2C_BUS, ADC2_ADDR) {
        Ok(_) => println!("✓ ADC2 test passed\n"),
        Err(e) => println!("✗ ADC2 test failed: {}\n", e),
    }
    
    // Continuous reading test
    println!("\nStarting continuous read test...");
    println!("Reading Channel 0 from both ADCs (Press Ctrl+C to stop)\n");
    
    let mut adc1 = Ads1015::new(I2C_BUS, ADC1_ADDR)?;
    let mut adc2 = Ads1015::new(I2C_BUS, ADC2_ADDR)?;
    
    for i in 0..20 {
        let raw1 = adc1.read_raw(Channel::Ain0, Gain::TwoThirds, DataRate::Sps1600)?;
        let volt1 = adc1.read_voltage(Channel::Ain0, Gain::TwoThirds)?;
        
        let raw2 = adc2.read_raw(Channel::Ain0, Gain::TwoThirds, DataRate::Sps1600)?;
        let volt2 = adc2.read_voltage(Channel::Ain0, Gain::TwoThirds)?;
        
        println!("Sample {:3}: ADC1 Ch0: {:5} raw ({:7.4}V)  |  ADC2 Ch0: {:5} raw ({:7.4}V)",
                 i + 1, raw1, volt1, raw2, volt2);
        
        thread::sleep(Duration::from_millis(200));
    }
    
    println!("\n✓ Test completed successfully!");
    println!("If values are changing, the ADCs are working correctly.");
    println!("If values are stuck at 0 or constant, check:");
    println!("  - I2C bus permissions (try: sudo chmod 666 /dev/i2c-2)");
    println!("  - Wiring connections (SDA, SCL, VDD, GND, ADDR pins)");
    println!("  - Pull-up resistors on I2C lines (typically 4.7kΩ)");
    println!("  - ADC power supply (3.3V or 5V depending on module)");
    
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn test_adc(bus: &str, address: u16) -> Result<()> {
    use i2cdev::core::I2CDevice;
    use i2cdev::linux::LinuxI2CDevice;
    
    // Test 1: Open device
    print!("  [1/4] Opening I2C device... ");
    let mut dev = LinuxI2CDevice::new(bus, address)?;
    println!("OK");
    
    // Test 2: Try to read config register
    print!("  [2/4] Reading config register... ");
    let config = dev.smbus_read_word_data(0x01)?;
    println!("OK (0x{:04X})", config);
    
    // Test 3: Try to read conversion register
    print!("  [3/4] Reading conversion register... ");
    let conv = dev.smbus_read_word_data(0x00)?;
    println!("OK (0x{:04X})", conv);
    
    // Test 4: Perform actual ADC read
    print!("  [4/4] Performing test conversion... ");
    let mut adc = Ads1015::new(bus, address)?;
    let raw = adc.read_raw(Channel::Ain0, Gain::TwoThirds, DataRate::Sps1600)?;
    let voltage = adc.read_voltage(Channel::Ain0, Gain::TwoThirds)?;
    println!("OK");
    println!("       Raw value: {} (0x{:04X})", raw, raw as u16);
    println!("       Voltage: {:.4}V", voltage);
    
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn main() {
    eprintln!("ERROR: This program requires Linux with I2C support.");
    std::process::exit(1);
}
