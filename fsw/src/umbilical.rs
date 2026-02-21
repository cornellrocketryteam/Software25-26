use embassy_executor::Spawner;
use embassy_sync::signal::Signal;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb::{UsbDevice, driver::EndpointError};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_time::Timer;

use crate::module::{self, UsbDriver};
use crate::constants;

/// Commands that can be received from the ground station via USB umbilical.
#[derive(Debug)]
pub enum UmbilicalCommand {
    Launch,
    OpenMav,
    CloseMav,
    OpenSv,
    CloseSv,
    Safe,
    ResetCard,
    ResetFram,
    Reboot,
}

/// Shared telemetry buffer: written by the flight loop, read by the sender task.
/// Uses Signal so the sender blocks until fresh data is available each cycle.
static TELEMETRY: Signal<CriticalSectionRawMutex, [u8; 80]> = Signal::new();

/// Command channel: receiver task pushes commands, flight loop polls them.
static COMMANDS: Channel<CriticalSectionRawMutex, UmbilicalCommand, 4> = Channel::new();

/// Called by the flight loop each cycle (after serializing the packet) to
/// provide fresh telemetry to the umbilical sender task.
pub fn update_telemetry(data: &[u8; 80]) {
    TELEMETRY.signal(*data);
}

/// Called by the flight loop each cycle to poll for incoming umbilical commands.
/// Returns `None` if no command is pending.
pub fn try_recv_command() -> Option<UmbilicalCommand> {
    COMMANDS.try_receive().ok()
}

/// Initialize USB subsystem: logger in debug mode, umbilical in release mode.
/// The RP2350 has a single USB peripheral, so only one can be active at a time.
pub fn setup(spawner: &Spawner, usb_driver: UsbDriver) {
    if cfg!(debug_assertions) {
        // Debug: USB -> logger for development
        spawner.spawn(logger_task(usb_driver).unwrap());
    } else {
        // Release: USB -> umbilical for flight/fill-station
        let (usb_device, usb_class) = module::init_usb_device(usb_driver);
        let (sender, receiver) = usb_class.split();
        spawner.spawn(usb_task(usb_device).unwrap());
        spawner.spawn(umbilical_sender_task(sender).unwrap());
        spawner.spawn(umbilical_receiver_task(receiver).unwrap());
    }
}

#[embassy_executor::task]
async fn logger_task(driver: UsbDriver) -> ! {
    embassy_usb_logger::run!({ constants::USB_LOGGER_BUFFER_SIZE }, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn usb_task(mut usb_device: UsbDevice<'static, UsbDriver>) -> ! {
    usb_device.run().await
}

#[embassy_executor::task]
async fn umbilical_sender_task(mut sender: Sender<'static, UsbDriver>) -> ! {
    loop {
        sender.wait_connection().await;

        loop {
            // Block until the flight loop provides fresh telemetry
            let data = TELEMETRY.wait().await;

            match sender.write_packet(&data).await {
                Ok(_) => {},
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow shouldn't be possible"),
                Err(EndpointError::Disabled) => break,
            };
        }
    }
}

#[embassy_executor::task]
async fn umbilical_receiver_task(mut receiver: Receiver<'static, UsbDriver>) -> ! {
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
            let cmd = match data {
                b"<L>" => Some(UmbilicalCommand::Launch),
                b"<M>" => Some(UmbilicalCommand::OpenMav),
                b"<m>" => Some(UmbilicalCommand::CloseMav),
                b"<S>" => Some(UmbilicalCommand::OpenSv),
                b"<s>" => Some(UmbilicalCommand::CloseSv),
                b"<V>" => Some(UmbilicalCommand::Safe),
                b"<D>" => Some(UmbilicalCommand::ResetCard),
                b"<F>" => Some(UmbilicalCommand::ResetFram),
                b"<R>" => Some(UmbilicalCommand::Reboot),

                b"<C1>" => None, // TODO: Change target lat (needs payload data)
                b"<C2>" => None, // TODO: Change target long (needs payload data)
                b"<C3>" => None, // TODO: Change ref pressure (needs payload data)
                b"<C4>" => None, // TODO: Change alt state (needs payload data)
                b"<C5>" => None, // TODO: Change card state (needs payload data)
                b"<C6>" => None, // TODO: Change alt armed (needs payload data)
                b"<C7>" => None, // TODO: Change flight mode (needs payload data)

                _ => None,
            };

            if let Some(c) = cmd {
                COMMANDS.try_send(c).ok();
            }

            // Only listen for an incoming umbilical packet every 100 ms
            Timer::after_millis(100).await;
        }
    }
}
