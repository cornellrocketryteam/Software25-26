use embassy_executor::Spawner;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb::{UsbDevice, driver::EndpointError};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_time::Timer;

use crate::module::{self, UsbDriver};
use crate::constants;
use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;

/// Sync header prepended to every telemetry frame so the receiver can find
/// packet boundaries in the byte stream.  Total frame = 2 + 80 = 82 bytes.
pub const SYNC_HEADER: [u8; 2] = [0xAA, 0x55];
const FRAME_SIZE: usize = SYNC_HEADER.len() + 80; // 82

/// Global software umbilical connection tracked by embassy-usb.
static IS_CONNECTED: AtomicBool = AtomicBool::new(false);

/// Simple atomic telemetry buffer - no Signal, no blocking
/// Main loop writes, sender task reads, both non-blocking
struct TelemetryBuffer {
    data: UnsafeCell<[u8; 80]>,
    ready: AtomicBool,
}

unsafe impl Sync for TelemetryBuffer {}

static TELEMETRY_BUF: TelemetryBuffer = TelemetryBuffer {
    data: UnsafeCell::new([0; 80]),
    ready: AtomicBool::new(false),
};

/// Returns whether the ground station umbilical is actively connected via USB.
pub fn is_connected() -> bool {
    IS_CONNECTED.load(Ordering::Relaxed)
}

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

/// Command channel: receiver task pushes commands, flight loop polls them.
static COMMANDS: Channel<CriticalSectionRawMutex, UmbilicalCommand, 4> = Channel::new();

/// Called by the flight loop to send telemetry
/// Non-blocking: always succeeds immediately, overwrites previous data if not sent yet
pub fn update_telemetry(data: &[u8; 80]) {
    unsafe {
        // Copy data to buffer
        (*TELEMETRY_BUF.data.get()).copy_from_slice(data);
        // Mark as ready
        TELEMETRY_BUF.ready.store(true, Ordering::Release);
    }
}

/// Called by the flight loop each cycle to poll for incoming umbilical commands.
/// Returns `None` if no command is pending.
pub fn try_recv_command() -> Option<UmbilicalCommand> {
    COMMANDS.try_receive().ok()
}

/// Initialize USB subsystem: logger in debug mode, umbilical in release mode.
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

/// Reads real telemetry from atomic buffer, prepends a 2-byte sync header,
/// and sends as two USB packets (64 + 18 bytes = 82-byte frame).
/// Only transmits when the flight loop has produced new data (no zero flooding).
#[embassy_executor::task]
async fn umbilical_sender_task(mut sender: Sender<'static, UsbDriver>) -> ! {
    loop {
        sender.wait_connection().await;
        IS_CONNECTED.store(true, Ordering::Relaxed);

        loop {
            // Only send when the flight loop has produced new data
            if TELEMETRY_BUF.ready.load(Ordering::Acquire) {
                let data = unsafe { *TELEMETRY_BUF.data.get() };
                TELEMETRY_BUF.ready.store(false, Ordering::Release);

                // Build framed packet: [0xAA, 0x55, ...80 bytes telemetry...] = 82 bytes
                let mut frame = [0u8; FRAME_SIZE];
                frame[0..2].copy_from_slice(&SYNC_HEADER);
                frame[2..82].copy_from_slice(&data);

                // Send first 64 bytes of frame
                match sender.write_packet(&frame[0..64]).await {
                    Ok(_) => {}
                    Err(EndpointError::BufferOverflow) => panic!("Buffer overflow on first chunk"),
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                };

                // Send remaining 18 bytes of frame
                match sender.write_packet(&frame[64..82]).await {
                    Ok(_) => {}
                    Err(EndpointError::BufferOverflow) => panic!("Buffer overflow on second chunk"),
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                };
            }

            Timer::after_millis(100).await;
        }
    }
}

#[embassy_executor::task]
async fn umbilical_receiver_task(mut receiver: Receiver<'static, UsbDriver>) -> ! {
    let mut buf = [0; 64];
    loop {
        receiver.wait_connection().await;
        IS_CONNECTED.store(true, Ordering::Relaxed);

        loop {
            let n = match receiver.read_packet(&mut buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow"),
                Err(EndpointError::Disabled) => {
                    IS_CONNECTED.store(false, Ordering::Relaxed);
                    break;
                }
            };

            let data = &buf[..n];

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
                _ => None,
            };

            if let Some(c) = cmd {
                COMMANDS.try_send(c).ok();
            }

            Timer::after_millis(100).await;
        }
    }
}
