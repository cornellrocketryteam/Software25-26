//! Cornell Rocketry Team Flight Software

#![no_std]
#![no_main]

mod driver;
mod module;
mod packet;
mod state;

use embassy_usb::{UsbDevice, driver::EndpointError};
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialize USB driver for logger
    let usb_driver = module::init_usb_driver(p.USB);
    let i2c_bus = module::init_shared_i2c(p.I2C0, p.PIN_0, p.PIN_1);
    let (spi, cs) = module::init_spi(p.SPI0, p.PIN_16, p.PIN_19, p.PIN_18, p.PIN_17, p.DMA_CH2, p.DMA_CH3);
    let uart = module::init_uart1(p.UART1, p.PIN_4, p.PIN_5, p.DMA_CH0, p.DMA_CH1);

    // Spawn USB logger task if we're in debug mode
    if cfg!(debug_assertions) {
        spawner.spawn(logger_task(usb_driver).unwrap());
    } else {
        let (usb_device, usb_class) = module::init_usb_device(usb_driver);
        spawner.spawn(usb_task(usb_device).unwrap());
        let (sender, receiver) = usb_class.split();
        spawner.spawn(umbilical_sender_task(sender).unwrap());
        spawner.spawn(umbilical_receiver_task(receiver).unwrap());
    }

    // GPIO 25 is the onboard LED
    let mut led = Output::new(p.PIN_25, Level::Low);

    let mut flight_state = state::FlightState::new(i2c_bus, spi, cs, uart).await;
    loop {
        flight_state.cycle_count += 1;

        flight_state.transition().await;
        flight_state.execute().await;

        log::info!(
            "Current Flight Mode: {} on cycle {}",
            flight_state.flight_mode_name(),
            flight_state.cycle_count
        );

        // Toggle LED
        led.toggle();
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::task]
async fn logger_task(driver: module::UsbDriver) -> ! {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn usb_task(mut usb_device: UsbDevice<'static, module::UsbDriver>) -> ! {
    usb_device.run().await
}

#[embassy_executor::task]
async fn umbilical_sender_task(mut sender: Sender<'static, module::UsbDriver>) -> ! {
    loop {
        // Cameron's implementation debounces the wait_connection to determine whether it is
        // actually connected or whether it is disconnected. embassy_usb is a little different,
        // because a "connection" is not a physical connection, rather it defines a connection as
        // having been enumerated and being ready to send/receive. I don't believe the debouncing
        // is necessary here, but it can be added

        sender.wait_connection().await;
        // buzzer.buzz_num(3)

        loop {
            let mut buf = [0; 30];
            for i in 0..buf.len() {
                buf[i] = i as u8;
            }

            // pack_data();
            //
            // memcpy(&packet[0], &packed_metadata, sizeof(uint16_t));
            // memcpy(&packet[2], &state::flight::timestamp, sizeof(uint32_t));
            // memcpy(&packet[6], &events, sizeof(uint32_t));
            //
            // memcpy(&packet[10], &state::adc::battery_voltage, sizeof(float));
            // memcpy(&packet[14], &state::adc::pressure_pt3, sizeof(float));
            // memcpy(&packet[18], &state::adc::pressure_pt4, sizeof(float));
            // memcpy(&packet[22], &state::adc::temp_rtd, sizeof(float));
            // memcpy(&packet[26], &state::alt::altitude, sizeof(float));

            match sender.write_packet(&buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow shouldn't be possible"),
                Err(EndpointError::Disabled) => break,
            };

            // Only write a outgoing umbilical packet every 100 ms
            Timer::after_millis(100).await;
        }

        // buzzer.buzz_num(2)
    }
}

#[embassy_executor::task]
async fn umbilical_receiver_task(mut receiver: Receiver<'static, module::UsbDriver>) -> ! {
    let mut buf = [0; 64];
    loop {
        receiver.wait_connection().await;

        loop {
            let n = match receiver.read_packet(&mut buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow isn't possible"),
                Err(EndpointError::Disabled) => break,
            };

            let data = &buf[..n];

            // https://github.com/cornellrocketryteam/Flight-Software24-25/blob/9e7f92667e200fd2deb381505554a37619a53eda/src/telem/telem.cpp#L124
            match data {
                b"<L>" => {}, // Launch
                b"<M>" => {}, // Open MAV
                b"<m>" => {}, // Close MAV
                b"<S>" => {}, // Open SV
                b"<s>" => {}, // Close SV

                b"<V>" => {}, // Safe

                b"<D>" => {}, // Reset card
                b"<F>" => {}, // Reset fram

                b"<R>" => {}, // Reboot

                b"<C1>" => {}, // Change target lat
                b"<C2>" => {}, // Change target long
                b"<C3>" => {}, // Change ref pressure
                b"<C4>" => {}, // Change alt state
                b"<C5>" => {}, // Change card state
                b"<C6>" => {}, // Change alt armed
                b"<C7>" => {}, // Change flight mode

                _ => {}, // Unknown command
            }

            // Only listen for an incoming umbilical packet every 100 ms
            Timer::after_millis(100).await;
        }
    }
}

