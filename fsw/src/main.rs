//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod constants;
mod driver;
mod module;
mod packet;
mod state;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // GPIO 25 (directly accessible on Pico 2, on WiFi chip for Pico 2 W)
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Initialize USB driver for logger
    let driver = module::init_usb_driver(p.USB);

    // Spawn USB logger task
    spawner.spawn(logger_task(driver).expect("logger task failed"));

    // Give USB time to enumerate and serial monitor to connect
    Timer::after_millis(5000).await;

    log::info!("=== FSW Starting ===");

    // Initialize flash
    let mut flash = module::init_onboard_flash(p.FLASH, p.DMA_CH4);

    // === Packet Flash Test ===
    log::info!("=== Packet Flash Test ===");

    // Create a test packet with known values
    let mut test_packet = packet::Packet::default();
    test_packet.metadata = 3;
    test_packet.ms_since_boot = 123456;
    test_packet.events = 0xABCD;
    test_packet.altitude = 1500.75;
    test_packet.temperature = 22.5;
    test_packet.latitude = 42_449_700;   // Ithaca ~42.4497 deg
    test_packet.longitude = -76_483_500; // Ithaca ~-76.4835 deg
    test_packet.satellites_in_view = 12;
    test_packet.unix_time = 1700000000;
    test_packet.horizontal_accuracy = 1500;
    test_packet.imu_accel_x = 0.1;
    test_packet.imu_accel_y = -0.2;
    test_packet.imu_accel_z = 9.81;
    test_packet.gyro_x = 1.5;
    test_packet.gyro_y = -2.3;
    test_packet.gyro_z = 0.7;
    test_packet.orientation_x = 10.0;
    test_packet.orientation_y = 20.0;
    test_packet.orientation_z = 30.0;
    test_packet.hi_g_accel_x = 0.0;
    test_packet.hi_g_accel_y = 0.0;
    test_packet.hi_g_accel_z = 1.0;
    test_packet.battery_voltage = 3.7;
    test_packet.pt3_pressure = 14.7;
    test_packet.pt4_pressure = 15.2;
    test_packet.rtd_temperature = 25.0;
    test_packet.motor_state = 0.0;

    // Write packet to flash
    log::info!("Writing 107-byte packet to flash...");
    match flash.write_packet(&test_packet).await {
        Ok(()) => log::info!("  Write OK"),
        Err(e) => log::error!("  Write FAILED: {:?}", e),
    }

    // Read packet back
    log::info!("Reading packet from flash...");
    match flash.read_packet().await {
        Ok(readback) => {
            log::info!("  Read OK");

            // Verify key fields
            log::info!("  sync_word:    0x{:08X} (expected 0x{:08X})", readback.sync_word, packet::SYNC_WORD);
            log::info!("  metadata:     {} (expected 3)", readback.metadata);
            log::info!("  ms_since_boot:{} (expected 123456)", readback.ms_since_boot);
            log::info!("  altitude:     {:.2} (expected 1500.75)", readback.altitude);
            log::info!("  temperature:  {:.1} (expected 22.5)", readback.temperature);
            log::info!("  latitude:     {} (expected 42449700)", readback.latitude);
            log::info!("  longitude:    {} (expected -76483500)", readback.longitude);
            log::info!("  satellites:   {} (expected 12)", readback.satellites_in_view);
            log::info!("  unix_time:    {} (expected 1700000000)", readback.unix_time);
            log::info!("  imu_accel_z:  {:.2} (expected 9.81)", readback.imu_accel_z);
            log::info!("  battery_v:    {:.1} (expected 3.7)", readback.battery_voltage);

            // Byte-level comparison
            let original_bytes = test_packet.to_bytes();
            let readback_bytes = readback.to_bytes();
            let mut mismatch = false;
            for i in 0..packet::Packet::SIZE {
                if original_bytes[i] != readback_bytes[i] {
                    log::error!("  MISMATCH at byte {}: wrote 0x{:02X}, read 0x{:02X}", i, original_bytes[i], readback_bytes[i]);
                    mismatch = true;
                }
            }
            if !mismatch {
                log::info!("  PASS: All 107 bytes match!");
            }
        }
        Err(e) => log::error!("  Read FAILED: {:?}", e),
    }

    log::info!("=== Packet Flash Test Complete ===");

    // Idle loop
    loop {
        led.toggle();
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::task]
async fn logger_task(driver: embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>) -> ! {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}
