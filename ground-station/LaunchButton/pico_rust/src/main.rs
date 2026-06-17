#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Create the driver
    let driver = Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Cornell Rocketry");
    config.product = Some("LaunchButton");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Create embassy-usb DeviceBuilder
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [], // no msos descriptors
        CONTROL_BUF.init([0; 64]),
    );

    // Create classes on the builder.
    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    let mut class = CdcAcmClass::new(&mut builder, state, 64);

    // Build the builder into a USB device
    let usb = builder.build();

    // Run the USB device in a separate task
    spawner.spawn(usb_task(usb).unwrap());

    // Pin 0 for the button, pulled down.
    let button = Input::new(p.PIN_0, Pull::Down);
    let mut prev_state = false;

    defmt::info!("Ready to launch!");

    loop {
        // We do not strictly wait for connection because we can just try writing.
        // But doing so avoids blocking the button loop if PC is disconnected.
        let current_state = button.is_high();
        if current_state && !prev_state {
            defmt::info!("Button pressed!");
            // Send <L> over USB CDC
            let _ = class.write_packet(b"<L>\n").await;
            // Small debounce delay
            Timer::after(Duration::from_millis(100)).await;
        }
        prev_state = current_state;
        Timer::after(Duration::from_millis(10)).await;
    }
}
