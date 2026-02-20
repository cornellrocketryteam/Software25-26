use embassy_usb::{UsbDevice, driver::EndpointError};
use crate::module::UsbDriver;
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_time::Timer;
use crate::module;

#[embassy_executor::task]
async fn logger_task(driver: module::UsbDriver) -> ! {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
pub async fn usb_task(mut usb_device: UsbDevice<'static, module::UsbDriver>) -> ! {
    usb_device.run().await
}

#[embassy_executor::task]
pub async fn umbilical_sender_task(mut sender: Sender<'static, UsbDriver>) -> ! {
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
pub async fn umbilical_receiver_task(mut receiver: Receiver<'static, UsbDriver>) -> ! {
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
                b"<L>" => { log::info!("UMBILICAL CMD: Launch"); },
                b"<M>" => { log::info!("UMBILICAL CMD: Open MAV"); },
                b"<m>" => { log::info!("UMBILICAL CMD: Close MAV"); },
                b"<S>" => { log::info!("UMBILICAL CMD: Open SV"); },
                b"<s>" => { log::info!("UMBILICAL CMD: Close SV"); },

                b"<V>" => { log::info!("UMBILICAL CMD: Safe"); },

                b"<D>" => { log::info!("UMBILICAL CMD: Reset Card"); },
                b"<F>" => { log::info!("UMBILICAL CMD: Reset FRAM"); },

                b"<R>" => { log::info!("UMBILICAL CMD: Reboot"); },

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


