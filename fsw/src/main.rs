//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::uart::{Async, UartRx};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

pub mod actuator;
mod constants;
mod driver;
mod flight_loop;
#[cfg(any(
    feature = "sim_simple",
    feature = "sim_fault",
    feature = "sim_stability",
    feature = "sim_extra",
    feature = "sim_flash",
    feature = "sim_hsim",
    feature = "sim_launch",
    feature = "sim_payload"
))]
#[path = "../Test/flight_sim.rs"]
mod flight_sim;
mod module;
mod packet;
mod state;
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

    let spi_bus =
        module::init_shared_spi(p.SPI0, p.PIN_4, p.PIN_3, p.PIN_2, p.DMA_CH2, p.DMA_CH3);
    let uart = module::init_uart0(p.UART0, p.PIN_30, p.PIN_31, p.DMA_CH0, p.DMA_CH1);
    let payload_uart = module::init_uart1(p.UART1, p.PIN_12, p.PIN_13, p.DMA_CH5, p.DMA_CH6);
    let (payload_tx, payload_rx) = payload_uart.split();

    #[cfg(feature = "test_payload_uart")]
    spawner.spawn(payload_loopback_task(payload_rx).unwrap());

    #[cfg(not(feature = "test_payload_uart"))]
    drop(payload_rx); // not needed in flight

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
        i2c_bus,
        spi_bus,
        fram_cs,
        altimeter_cs,
        arming_switch,
        umbilical_sense,
        uart,
        ssa,
        buzzer,
        mav,
        sv,
        flash,
        payload_tx,
    )
    .await;
    log::info!("Flight State Initialized.");

    // Reset FRAM for testing (COMMENT OUT FOR REAL FLIGHT)
    flight_state.reset_fram().await;

    // --- FLIGHT SIMULATIONS --- //
    #[cfg(any(
        feature = "sim_simple",
        feature = "sim_fault",
        feature = "sim_stability",
        feature = "sim_extra",
        feature = "sim_flash",
        feature = "sim_launch",
        feature = "sim_hsim",
        feature = "sim_payload"
    ))]
    let mut flight_state = {
        let mut flight_loop = flight_loop::FlightLoop::new(flight_state);

        #[cfg(feature = "sim_simple")]
        {
            log::info!("Starting Flight Simulation (Simple)...");
            flight_sim::simulate_flight_simple(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_launch")]
        flight_sim::simulate_launch_sequence(&mut flight_loop).await;

        #[cfg(feature = "sim_fault")]
        {
            log::info!("Starting Flight Simulation (Fault)...");
            flight_sim::simulate_fault_scenarios(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_stability")]
        {
            log::info!("Starting Flight Simulation (Stability)...");
            flight_sim::simulate_stability_scenarios(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_extra")]
        {
            log::info!("Starting Flight Simulation (Extra Features)...");
            flight_sim::simulate_extra_features(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_flash")]
        {
            log::info!("Starting Flight Simulation (QSPI Flash Storage)...");
            flight_sim::simulate_flash_storage(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_hsim")]
        {
            log::info!("Starting Hardware-in-the-Loop Flight Simulation (HSIM)...");
            flight_sim::simulate_flight_hsim(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(feature = "sim_payload")]
        {
            log::info!("Starting Payload Ground Command Simulation...");
            flight_sim::simulate_payload_commands(&mut flight_loop).await;
            log::info!("Simulation Complete.");
        }

        #[cfg(not(any(
            feature = "test_mav",
            feature = "test_sv",
            feature = "test_ssa",
            feature = "test_sensors",
            feature = "test_buzzer",
            feature = "test_all"
        )))]
        {
            log::info!("All Simulations Complete.");
            loop {
                Timer::after_millis(1000).await;
            }
        }

        flight_loop.flight_state
    };

    // --- HARDWARE TEST MODES --- //

    #[cfg(feature = "test_mav")]
    {
        log::info!("Starting MAV Test Mode...");
        loop {
            log::info!(
                "Actuating MAV OPEN for {}ms",
                constants::MAV_OPEN_DURATION_MS
            );
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
        loop {
            log::info!("Actuating SV OPEN for 2000ms");
            flight_state.open_sv(2000).await;
            flight_state.update_actuators().await;
            Timer::after_millis(4000).await;

            log::info!("Actuating SV CLOSE");
            flight_state.close_sv().await;
            flight_state.update_actuators().await;
            Timer::after_millis(5000).await;
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

    #[cfg(feature = "test_flash")]
    {
        log::info!("Starting QSPI Flash STRESS TEST Mode...");
        let mut loop_handler = flight_loop::FlightLoop::new(flight_state);
        let mut test_counter = 0;

        loop {
            log::info!("--- Stress Test Cycle {} ---", test_counter);

            // Write 10 packets rapidly with varying data
            for i in 0..10 {
                loop_handler.flight_state.packet.altitude = 100.0 * test_counter as f32 + i as f32;
                loop_handler.flight_state.packet.temp = 20.0 + (i as f32 * 0.5);
                loop_handler.flight_state.packet.pressure = 101325.0 - (test_counter as f32 * 10.0);
                loop_handler.flight_state.packet.flight_mode = (i % 7) as u32;

                loop_handler.flight_state.save_packet_to_flash().await;
            }

            // Report status
            loop_handler.flight_state.print_flash_status().await;

            // Poll for commands while waiting
            log::info!("Waiting for umbilical commands (e.g. <G>, <W>, <I>)...");
            for _ in 0..20 {
                loop_handler.check_umbilical_commands().await;
                Timer::after_millis(100).await;
            }

            led.toggle();
            test_counter += 1;
        }
    }

    #[cfg(all(
        any(feature = "test_all", feature = "test_hw_all"),
        not(feature = "test_flash")
    ))]
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

    #[cfg(feature = "test_payload_uart")]
    {
        log::info!("=== Payload UART Loopback Test ===");
        log::info!("Hardware: Connect GPIO 12 (TX) -> GPIO 13 (RX) with a jumper.");

        let mut flight_loop = flight_loop::FlightLoop::new(flight_state);
        // N1 only triggers in Startup/Standby
        flight_loop.set_flight_mode(state::FlightMode::Startup);

        let mut test_cycle: u32 = 0;
        loop {
            let cmd = match test_cycle % 4 {
                0 => {
                    log::info!("[TEST] Injecting N1 (Camera Deploy)");
                    crate::umbilical::UmbilicalCommand::PayloadN1
                }
                1 => {
                    log::info!("[TEST] Injecting N2");
                    crate::umbilical::UmbilicalCommand::PayloadN2
                }
                2 => {
                    log::info!("[TEST] Injecting N3");
                    crate::umbilical::UmbilicalCommand::PayloadN3
                }
                _ => {
                    log::info!("[TEST] Injecting N4");
                    crate::umbilical::UmbilicalCommand::PayloadN4
                }
            };

            // Simulate ground station pressing button -> USB -> umbilical queue
            crate::umbilical::push_command(cmd);

            // Run one flight loop tick: check_umbilical_commands will dequeue
            // the command and write e.g. "N1\n" to GPIO 12.
            // The spawned payload_loopback_task reads GPIO 13 and logs SUCCESS.
            flight_loop.execute().await;

            test_cycle += 1;
            led.toggle();
            Timer::after_secs(3).await;
        }
    }

    // --- NORMAL FLIGHT LOOP --- //
    #[cfg(not(any(
        feature = "test_mav",
        feature = "test_sv",
        feature = "test_ssa",
        feature = "test_sensors",
        feature = "test_buzzer",
        feature = "test_radio",
        feature = "test_all",
        feature = "test_hw_all",
        feature = "test_payload_uart",
        feature = "sim_simple",
        feature = "sim_fault",
        feature = "sim_stability",
        feature = "sim_extra",
        feature = "sim_flash",
        feature = "sim_launch",
        feature = "sim_hsim",
        feature = "sim_payload"
    )))]
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

/// Reads every byte that arrives on the RX pin and logs it.
/// When GPIO 32 (TX) is jumpered to GPIO 33 (RX) this confirms the
/// real payload signal (e.g. "N1\n") was successfully transmitted.
#[embassy_executor::task]
async fn payload_loopback_task(mut rx: UartRx<'static, Async>) -> ! {
    log::info!("LOOPBACK: Monitor task running. Waiting for signals...");
    // Signals are "N1\n", "N2\n", "N3\n", "N4\n" — all 3 bytes
    let mut buf = [0u8; 3];
    loop {
        match embassy_time::with_timeout(embassy_time::Duration::from_secs(5), rx.read(&mut buf))
            .await
        {
            Ok(Ok(())) => {
                let s = core::str::from_utf8(&buf).unwrap_or("<?>");
                log::info!("LOOPBACK SUCCESS: [{}]", s.trim());
            }
            Ok(Err(e)) => log::error!("LOOPBACK RX error: {:?}", e),
            Err(_) => log::warn!("LOOPBACK: no data in 5s — is the jumper connected?"),
        }
    }
}
