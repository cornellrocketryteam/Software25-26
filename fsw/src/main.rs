//! Cornell Rocketry Team Flight Software
//! Rust implementation using Embassy async framework
//! Based on Flight-Software24-25 architecture

#![no_std]
#![no_main]

extern crate alloc;

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{Config as I2cConfig, I2c, InterruptHandler};
use embassy_rp::peripherals::{I2C0, I2C1};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

mod actuator;
mod constants;
mod flight;
mod sensor;
mod state;
mod telem;

use flight::FlightController;

// Global allocator for heap memory (required by ublox crate)
use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

// Bind I2C interrupt handlers
bind_interrupts!(struct Irqs0 {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

bind_interrupts!(struct Irqs1 {
    I2C1_IRQ => InterruptHandler<I2C1>;
});

// Program metadata for `picotool info`
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Cornell Rocketry FSW"),
    embassy_rp::binary_info::rp_program_description!(
        c"Flight Software for Cornell Rocketry Team - Rust/Embassy Implementation"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize the heap
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 8192;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe {
            let heap_ptr = core::ptr::addr_of_mut!(HEAP_MEM);
            HEAP.init((*heap_ptr).as_ptr() as usize, HEAP_SIZE)
        }
    }

    info!("=== Cornell Rocketry Flight Software Starting ===");

    // Initialize peripherals with default config
    let p = embassy_rp::init(Default::default());

    // Initialize status LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Blink LED to indicate startup
    for _ in 0..3 {
        led.set_high();
        Timer::after_millis(100).await;
        led.set_low();
        Timer::after_millis(100).await;
    }

    info!("Initializing I2C bus for sensors...");

    // Configure I2C for sensors (400kHz)
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 400_000;

    // Initialize I2C bus (adjust pins as needed for your hardware)
    // Using I2C0: SDA=PIN_4, SCL=PIN_5
    let mut i2c = I2c::new_async(
        p.I2C0, p.PIN_5, // SCL
        p.PIN_4, // SDA
        Irqs0, i2c_config,
    );

    info!("Initializing GPIO for actuators...");

    // Initialize actuator pins (adjust as needed)
    let drogue_pin = Output::new(p.PIN_16, Level::Low);
    let main_pin = Output::new(p.PIN_17, Level::Low);

    info!("Creating flight controller...");

    // Create flight controller
    let mut flight_controller = FlightController::new();

    // Initialize flight controller
    match flight_controller.init(&mut i2c).await {
        Ok(_) => info!("Flight controller initialized successfully"),
        Err(e) => {
            error!("Failed to initialize flight controller: {}", e);
            core::panic!("Initialization failed");
        }
    }

    // Initialize actuators
    flight_controller.init_actuators(drogue_pin, main_pin);

    info!("=== Entering Main Flight Loop ===");

    // Main flight loop
    loop {
        // Heartbeat LED
        led.toggle();

        // Execute one flight control cycle
        flight_controller.execute(&mut i2c).await;
    }
}
