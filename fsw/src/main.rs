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
mod flight_loop;
pub mod actuator;
pub mod umbilical;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize USB subsystem (umbilical in release mode)
    let usb_driver = module::init_usb_driver(p.USB);
    umbilical::setup(&spawner, usb_driver);

    // Give logger a moment to attach in debug mode
    if cfg!(debug_assertions) {
        Timer::after_millis(5000).await;
    }
    log::info!("Booting Cornell Rocketry FSW...");
    Timer::after_millis(1000).await;
    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    
    // Perform an I2C scan
    /*
    log::info!("Scanning I2C Bus...");
    {
        use embedded_hal_async::i2c::I2c;
        let mut bus = i2c_bus.lock().await;
        let mut found = 0;
        for addr in 0x08u8..=0x77u8 {
            let mut buf = [0u8; 1];
            // Some I2C peripherals optimize away 0-byte reads. We request 1 byte.
            match bus.read(addr, &mut buf).await {
                Ok(_) => {
                    log::info!("Found I2C device at address: {:#04x}", addr);
                    found += 1;
                }
                Err(_) => {}
            }
        }
        log::info!("I2C scan complete. Found {} devices.", found);
    }
    */
    
    let spi_bus = module::init_shared_spi(
        p.SPI0, p.PIN_4, p.PIN_3, p.PIN_2, p.DMA_CH2, p.DMA_CH3,
    );
    let uart = module::init_uart0(p.UART0, p.PIN_30, p.PIN_31, p.DMA_CH0, p.DMA_CH1);

    let fram_cs = Output::new(p.PIN_17, Level::High);
    let _flash_cs = Output::new(p.PIN_6, Level::High); // For onboard flash
    let altimeter_cs = Output::new(p.PIN_7, Level::High);

    // Onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Arming Switch and Umbilical Sense
    let arming_switch = embassy_rp::gpio::Input::new(p.PIN_10, embassy_rp::gpio::Pull::Down);
    let umbilical_sense = embassy_rp::gpio::Input::new(p.PIN_24, embassy_rp::gpio::Pull::Down);

    // Actuators
    let (ssa, buzzer, mav, sv) = module::init_actuators(
        p.PIN_36,
        p.PIN_39,
        p.PIN_21,
        p.PWM_SLICE8,
        p.PIN_40,
        p.PIN_47,
    );
    let flash = module::init_onboard_flash(p.FLASH, p.DMA_CH4);
    
    log::info!("Initializing Flight State (Sensors & Actuators)...");
    let mut flight_state = state::FlightState::new(
        i2c_bus, spi_bus, fram_cs, altimeter_cs, arming_switch, umbilical_sense, uart,
        ssa,
        buzzer,
        mav,
        sv,
        flash,
    ).await;
    log::info!("Flight State Initialized.");

    // Reset FRAM for testing (COMMENT OUT FOR REAL FLIGHT)
    flight_state.reset_fram().await;

    // --- HARDWARE TEST MODES --- //

    #[cfg(feature = "test_mav")]
    {
        log::info!("Starting MAV Test Mode...");
        loop {
            log::info!("Actuating MAV OPEN for {}ms", constants::MAV_OPEN_DURATION_MS);
            flight_state.open_mav(constants::MAV_OPEN_DURATION_MS).await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::MAV_OPEN_DURATION_MS + 2000).await;
            
            log::info!("Actuating MAV CLOSE");
            flight_state.close_mav().await;
            flight_state.update_actuators().await;
            Timer::after_millis(5000).await;
        }
    }

    #[cfg(feature = "test_sv")]
    {
        log::info!("Starting SV Test Mode...");
        log::info!("SV will cycle: ON for {}ms, OFF for 30s", constants::SV_TEST_DURATION_MS);
        loop {
            log::info!("Actuating SV OPEN for {}ms", constants::SV_TEST_DURATION_MS);
            flight_state.open_sv(0).await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::SV_TEST_DURATION_MS).await;

            log::info!("Actuating SV CLOSE");
            flight_state.close_sv().await;
            flight_state.update_actuators().await;

            log::info!("Waiting 30 seconds...");
            Timer::after_millis(30_000).await;
        }
    }

    #[cfg(feature = "test_ssa")]
    {
        log::info!("Starting SSA Test Mode (Drogue and Main)...");
        loop {
            log::info!("Firing Drogue SSA for {}ms", constants::SSA_THRESHOLD_MS);
            flight_state.trigger_drogue().await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::SSA_THRESHOLD_MS + 2000).await;
            
            log::info!("Firing Main SSA for {}ms", constants::SSA_THRESHOLD_MS);
            flight_state.trigger_main().await;
            flight_state.update_actuators().await;
            Timer::after_millis(5000).await;
        }
    }

    #[cfg(feature = "test_buzzer")]
    {
        log::info!("Starting Buzzer Test Mode...");
        loop {
            log::info!("Actuating Buzzer 3 times");
            flight_state.buzz(3);
            // Loop calling update_actuators repeatedly to allow the buzzer state machine to process buzzes
            for _ in 0..20 {
                flight_state.update_actuators().await;
                Timer::after_millis(50).await;
            }
            log::info!("Wait 2 seconds...");
            Timer::after_millis(2000).await;
            log::info!("Actuating Buzzer 2 times");
            flight_state.buzz(2);
            for _ in 0..15 {
                flight_state.update_actuators().await;
                Timer::after_millis(50).await;
            }
            log::info!("Wait 5 seconds...");
            Timer::after_millis(5000).await;
        }
    }

    #[cfg(feature = "test_sensors")]
    {
        log::info!("Starting Sensor Test Mode...");
        loop {
            flight_state.read_sensors().await;
            flight_state.cycle_count += 1;
            led.toggle();
            Timer::after_millis(500).await; // Fast loop for sensor readouts
        }
    }


    #[cfg(feature = "test_radio")]
    {
        log::info!("Starting Radio Test Mode...");
        loop {
            // 1. Send data
            flight_state.read_sensors().await;
            log::info!("Transmitting test packet over radio");
            flight_state.transmit().await;

            // 2. Listen for response
            log::info!("Listening for 5000ms...");
            let mut buf = [0u8; 32];
            // Read with timeout so we don't block forever
            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(5000),
                flight_state.receive_radio(&mut buf),
            )
            .await
            {
                Ok(Ok(_)) => {
                    log::info!("Received data! Raw bytes: {:?}", &buf);
                }
                Ok(Err(e)) => log::warn!("Radio receive error: {:?}", e),
                Err(_) => log::info!("No data received (timeout)"),
            }

            flight_state.cycle_count += 1;
            led.toggle();
        }
    }


    #[cfg(feature = "test_all")]
    {
        log::info!("Starting Combined Hardware Test Sequence...");
        let mut test_cycle = 0;
        loop {
            log::info!("--- Combined Test Cycle {} ---", test_cycle);
            
            // 1. Read Sensors
            log::info!("1. Testing Sensors...");
            flight_state.read_sensors().await;
            Timer::after_millis(1000).await;

            // 2. Test MAV
            log::info!("2. Testing MAV...");
            flight_state.open_mav(constants::MAV_OPEN_DURATION_MS).await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::MAV_OPEN_DURATION_MS + 1000).await;
            flight_state.close_mav().await;
            flight_state.update_actuators().await;
            Timer::after_millis(1000).await;

            // 3. Test SV
            log::info!("3. Testing SV...");
            flight_state.open_sv(1000).await;
            flight_state.update_actuators().await;
            Timer::after_millis(2000).await;
            flight_state.close_sv().await;
            flight_state.update_actuators().await;
            Timer::after_millis(1000).await;

            // 4. Test SSA (Drogue only to save time)
            log::info!("4. Testing Drogue SSA...");
            flight_state.trigger_drogue().await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::SSA_THRESHOLD_MS + 1000).await;

            // 5. Test Buzzer
            log::info!("5. Testing Buzzer 3 times...");
            flight_state.buzz(3);
            for _ in 0..20 {
                flight_state.update_actuators().await;
                Timer::after_millis(50).await;
            }
            Timer::after_millis(2000).await;
            log::info!("Actuating Buzzer 2 times");
            flight_state.buzz(2);
            for _ in 0..15 {
                flight_state.update_actuators().await;
                Timer::after_millis(50).await;
            }

            test_cycle += 1;
            led.toggle();
            log::info!("Cycle complete. Waiting 3 seconds before repeating...");
            Timer::after_millis(3000).await;
        }
    }

    // --- NORMAL FLIGHT LOOP --- //
    #[cfg(not(any(feature = "test_mav", feature = "test_sv", feature = "test_ssa", feature = "test_sensors", feature = "test_buzzer", feature = "test_all")))]
    {
        let mut flight_loop = flight_loop::FlightLoop::new(flight_state);
        
        // STEP 4: Run actual flight loop with real telemetry
        loop {
            flight_loop.flight_state.cycle_count += 1;
            flight_loop.execute().await;
    
            // Toggle LED for heartbeat (every 10 cycles = 1 Hz blink at 10 Hz loop)
            if flight_loop.flight_state.cycle_count % 10 == 0 {
                led.toggle();
            }
            Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
        }
    }
}
