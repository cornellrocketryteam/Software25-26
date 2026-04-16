//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

use embassy_executor::{Executor, Spawner};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::uart::{Async, UartRx};
use embassy_time::{Duration, Instant, Timer};
use embassy_rp::watchdog::Watchdog;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

pub mod actuator;
pub mod airbrake_task;
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

// Stack and executor for Core 1 (airbrake controller)
static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Spawn the airbrake controller on Core 1.
    // It blocks on AIRBRAKE_INPUT signal until the flight loop sends data.
    spawn_core1(p.CORE1, CORE1_STACK.init(Stack::new()), move || {
        let executor = CORE1_EXECUTOR.init(Executor::new());
        executor.run(|spawner| {
            spawner.spawn(airbrake_task::airbrake_core1_task().unwrap());
        });
    });

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

    let spi_bus = module::init_shared_spi(p.SPI0, p.PIN_4, p.PIN_3, p.PIN_2, p.DMA_CH2, p.DMA_CH3);
    let uart = module::init_uart1(p.UART1, p.PIN_8, p.PIN_9, p.DMA_CH0, p.DMA_CH1);
    let payload_uart = module::init_uart0(p.UART0, p.PIN_32, p.PIN_33, p.DMA_CH5, p.DMA_CH6);
    let (payload_tx, payload_rx) = payload_uart.split();

    #[cfg(feature = "test_payload_uart")]
    spawner.spawn(payload_loopback_task(payload_rx).unwrap());

    #[cfg(not(feature = "test_payload_uart"))]
    drop(payload_rx); // not needed in flight

    let fram_cs = Output::new(p.PIN_17, Level::High);
    let altimeter_cs = Output::new(p.PIN_7, Level::High);

    // Onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Arming Switch and Umbilical Sense
    let arming_switch = embassy_rp::gpio::Input::new(p.PIN_10, embassy_rp::gpio::Pull::Down);
    let umbilical_sense = embassy_rp::gpio::Input::new(p.PIN_24, embassy_rp::gpio::Pull::Down);

    // CFC_ARM (GPIO 41): off-board arming signal, input with pull-down
    let cfc_arm = embassy_rp::gpio::Input::new(p.PIN_41, embassy_rp::gpio::Pull::Down);

    // CFC_ARM_Indicator (GPIO 21): PWM at 400 Hz, drives buzzer + LED on-board
    let (ssa, buzzer, mav, sv) = module::init_actuators(
        p.PIN_36,
        p.PIN_39,
        p.PWM_SLICE2, // CFC_ARM_Indicator buzzer slice
        p.PIN_21,      // CFC_ARM_Indicator pin
        p.PWM_SLICE8,
        p.PIN_40,
        p.PIN_47,
    );
    let flash_cs = Output::new(p.PIN_6, embassy_rp::gpio::Level::High);
    let flash = module::init_onboard_flash(spi_bus, flash_cs);
    let airbrake_system = module::init_airbrake(p.PIN_37, p.PWM_SLICE11, p.PIN_38);

    log::info!("Initializing Flight State (Sensors & Actuators)...");
    let mut flight_state = state::FlightState::new(
        i2c_bus,
        spi_bus,
        fram_cs,
        altimeter_cs,
        arming_switch,
        umbilical_sense,
        cfc_arm,
        uart,
        ssa,
        buzzer,
        mav,
        sv,
        airbrake_system,
        flash,
        payload_tx,
    )
    .await;
    log::info!("Flight State Initialized.");

    // Reset FRAM for testing (COMMENT OUT FOR REAL FLIGHT)
    //flight_state.reset_fram().await;

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
            log::info!("Firing Drogue SSA for {}ms", constants::SSA_THRESHOLD_MS);
            flight_state.trigger_drogue().await;
            flight_state.update_actuators().await;
            Timer::after_millis(constants::SSA_THRESHOLD_MS + 1000).await;
            
            log::info!("Firing Main SSA for {}ms", constants::SSA_THRESHOLD_MS);
            flight_state.trigger_main().await;
            flight_state.update_actuators().await;
            Timer::after_millis(5000).await;
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

    #[cfg(feature = "test_radio_tx")]
    {
        log::info!("Starting Radio Test Mode (TX + RX)...");
        let mut test_counter: f32 = 0.0;
        loop {
            // Heartbeat toggle at start of loop
            led.toggle();

            // 1. Send data
            flight_state.read_sensors().await;

            // Inject dummy data so we can see something even if sensors are missing
            flight_state.packet.latitude = 42.44; // Ithaca
            flight_state.packet.longitude = -76.50;
            flight_state.packet.timestamp = test_counter;

            log::info!(
                "Transmitting telemetry packet (counter={})...",
                test_counter
            );
            flight_state.transmit().await;

            // 2. Listen for response
            log::info!("Listening for 500ms...");
            let mut buf = [0u8; crate::packet::Packet::SIZE];
            // Read with shorter timeout so the loop stays responsive
            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(500),
                flight_state.receive_radio(&mut buf),
            )
            .await
            {
                Ok(Ok(_)) => {
                    let p = crate::packet::Packet::from_bytes(&buf);
                    log::info!(
                        "Received Packet! Alt: {:.2}m, Mode: {}",
                        p.altitude,
                        p.flight_mode
                    );
                }
                Ok(Err(e)) => log::warn!("Radio receive error: {:?}", e),
                Err(_) => {} // Silent timeout
            }

            flight_state.cycle_count += 1;
            test_counter += 1.0;
            Timer::after_millis(500).await;
        }
    }

    #[cfg(feature = "test_radio_rx")]
    {
        log::info!("Starting Radio RECEIVE ONLY mode...");
        log::info!("Waiting for telemetry packets from another board...");
        loop {
            match flight_state.receive_telemetry().await {
                Ok(p) => {
                    log::info!("--- PACKET RECEIVED ---");
                    log::info!(
                        "Mode: {}, Alt: {:.2}m, Temp: {:.2}C",
                        p.flight_mode,
                        p.altitude,
                        p.temp
                    );
                    log::info!(
                        "Accel: X={:.2} Y={:.2} Z={:.2}",
                        p.accel_x,
                        p.accel_y,
                        p.accel_z
                    );
                    log::info!("Actuators: MAV={}, SV={}", p.mav_open, p.sv_open);
                    led.toggle();
                }
                Err(e) => {
                    log::warn!("Radio receive error: {:?}", e);
                }
            }
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

    // --- AIRBRAKE ACTUATION TEST --- //
    // Sweeps deployment 0% → 25% → 50% → 75% → 100% → 0%, holding each
    // position for 2 seconds so you can observe the motor response.
    // Logs the deployment % and the resulting pulse width (µs) at each step.
    //
    // Flash with:  cargo build --release --features test_airbrakes
    //
    // What to check on an oscilloscope:
    //   GPIO 38: 50 Hz signal, pulse width stepping through:
    //     0%   → 1000 µs
    //     25%  → 1250 µs
    //     50%  → 1500 µs
    //     75%  → 1750 µs
    //     100% → 2000 µs
    //     0%   → 1000 µs  (retract)
    #[cfg(feature = "test_airbrakes")]
    {
        let mut flight_state = flight_state; // rebind as mutable for direct access
        log::info!("=== Airbrake Actuation Test ===");
        log::info!("GPIO 37 = ENABLE (going HIGH now)");
        log::info!("GPIO 38 = PWM signal (50 Hz, 1000-2000 us)");

        // Enable the ODrive — GPIO 37 HIGH
        flight_state.airbrake_system.enable();
        // Give the ODrive 1 second to power up and recognise the enable line
        Timer::after_millis(6000).await;

        // Steps: (deployment_fraction, label)
        let steps: [(f32, &str); 6] = [
            (0.00, "0%   → 1000 us (fully retracted)"),
            (0.25, "25%  → 1250 us"),
            (0.50, "50%  → 1500 us (mid)"),
            (0.75, "75%  → 1750 us"),
            (1.00, "100% → 2000 us (fully deployed)"),
            (0.00, "0%   → 1000 us (retract — SAFE END)"),
        ];

        for (dep, label) in steps {
            log::info!("Setting airbrake deployment: {}", label);
            flight_state.airbrake_system.set_deployment(dep);
            led.toggle();
            // Hold for 2 seconds — long enough to see panel movement
            Timer::after_millis(2000).await;
        }

        // Disable ODrive after test — GPIO 37 LOW
        flight_state.airbrake_system.disable();
        log::info!("Test complete. ODrive disabled (GPIO 37 LOW).");
        log::info!("Airbrakes should be fully retracted.");

        // Sit idle — reflash to exit
        loop {
            Timer::after_millis(1000).await;
        }
    }

    // --- WATCHDOG TEST --- //
    // Verifies the hardware watchdog fires correctly when execute() hangs.
    //
    // Flash with:  cargo build --release --features test_watchdog
    //
    // Part 1 — 5 normal flight loop cycles with the watchdog active.
    //   Each cycle's elapsed time is logged. If the watchdog fires here,
    //   execute() is already overrunning in normal conditions — investigate.
    //
    // Part 2 — deliberate 200 ms stall without feeding the watchdog.
    //   The chip must reset within WATCHDOG_TIMEOUT_MS (120 ms) of the last feed.
    //   You will see the board reboot (boot message re-appears in the USB logger).
    //   The "WATCHDOG FAILED" line after the stall must NEVER appear — if it
    //   does, the watchdog is not working.
    #[cfg(feature = "test_watchdog")]
    {
        const NORMAL_CYCLES: u32 = 5;

        log::info!("=== Watchdog Test ===");
        log::info!(
            "Watchdog timeout: {} ms  |  Budget: {} ms  |  Test stall: {} ms",
            constants::WATCHDOG_TIMEOUT_MS,
            constants::LOOP_BUDGET_MS,
            constants::WATCHDOG_TEST_STALL_MS,
        );

        let mut flight_loop = flight_loop::FlightLoop::new(flight_state);
        let mut watchdog = Watchdog::new(p.WATCHDOG);
        watchdog.start(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));

        // Part 1: normal cycles — watchdog must not fire
        log::info!("Part 1: running {} normal cycles...", NORMAL_CYCLES);
        for cycle in 0..NORMAL_CYCLES {
            watchdog.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));
            let start = Instant::now();
            flight_loop.execute().await;
            let elapsed = start.elapsed().as_millis();
            watchdog.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64)); // cover the sleep below
            log::info!(
                "  Cycle {}/{}: {} ms (budget: {} ms) {}",
                cycle + 1,
                NORMAL_CYCLES,
                elapsed,
                constants::LOOP_BUDGET_MS,
                if elapsed > constants::LOOP_BUDGET_MS { "OVERRUN" } else { "OK" },
            );
            led.toggle();
            Timer::after_millis(constants::MAIN_LOOP_DELAY_MS).await;
        }

        // Part 2: deliberate stall — watchdog MUST fire and reset the chip
        log::info!(
            "Part 2: stalling for {} ms — chip must reset in ~{} ms...",
            constants::WATCHDOG_TEST_STALL_MS,
            constants::WATCHDOG_TIMEOUT_MS,
        );
        watchdog.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64)); // arm the countdown one last time
        // No more feeds — the chip resets mid-sleep
        Timer::after_millis(constants::WATCHDOG_TEST_STALL_MS).await;

        // If execution reaches here the watchdog did NOT fire — that is a failure
        log::error!("WATCHDOG FAILED — chip did not reset after {} ms stall!", constants::WATCHDOG_TEST_STALL_MS);
        loop {
            Timer::after_millis(500).await;
            led.toggle();
        }
    }

    // --- NORMAL FLIGHT LOOP --- //
    #[cfg(not(any(
        feature = "test_mav",
        feature = "test_sv",
        feature = "test_ssa",
        feature = "test_sensors",
        feature = "test_buzzer",
        feature = "test_radio_tx",
        feature = "test_radio_rx",
        feature = "test_all",
        feature = "test_hw_all",
        feature = "test_payload_uart",
        feature = "test_airbrakes",
        feature = "test_watchdog",
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
        let mut watchdog = Watchdog::new(p.WATCHDOG);
        watchdog.start(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));

        loop {
            flight_loop.flight_state.cycle_count += 1;

            // feed before execute() and starts the countdown
            // If execute() hangs for > WATCHDOG_TIMEOUT_MS, chip resets
            watchdog.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));
            let start = Instant::now();

            flight_loop.execute().await;

            let elapsed = start.elapsed().as_millis();

            // Feed again immediately after execute() before sleep
            // This prevents the 50 ms Timer delay from counting toward the timeout
            watchdog.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));

            if elapsed > constants::LOOP_BUDGET_MS {
                log::warn!(
                    "Loop overrun: execute() took {} ms (budget: {} ms)",
                    elapsed,
                    constants::LOOP_BUDGET_MS,
                );
            }

            // Toggle LED for heartbeat (every 20 cycles = 1 Hz blink at 20 Hz loop)
            if flight_loop.flight_state.cycle_count % 20 == 0 {
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
