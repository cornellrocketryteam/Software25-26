use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_usb::{UsbDevice, driver::EndpointError};

use crate::constants;
use crate::module::{self, UsbDriver};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// Sync header prepended to every telemetry frame so the receiver can find
/// packet boundaries in the byte stream.  Total frame = 2 + 82 = 84 bytes.
pub const SYNC_HEADER: [u8; 2] = [0xAA, 0x55];
const FRAME_SIZE: usize = SYNC_HEADER.len() + crate::packet::Packet::SIZE; // 84

/// Global software umbilical connection tracked by embassy-usb.
static IS_CONNECTED: AtomicBool = AtomicBool::new(false);

/// Simple atomic telemetry buffer - no Signal, no blocking
/// Main loop writes, sender task reads, both non-blocking
struct TelemetryBuffer {
    data: UnsafeCell<[u8; crate::packet::Packet::SIZE]>,
    ready: AtomicBool,
}

unsafe impl Sync for TelemetryBuffer {}

static TELEMETRY_BUF: TelemetryBuffer = TelemetryBuffer {
    data: UnsafeCell::new([0; crate::packet::Packet::SIZE]),
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
    DumpFlash,
    WipeFlash,
    FlashInfo,
    PayloadN1,
    PayloadN2,
    PayloadN3,
    PayloadN4,
}

/// Command channel: receiver task pushes commands, flight loop polls them.
static COMMANDS: Channel<CriticalSectionRawMutex, UmbilicalCommand, 4> = Channel::new();

/// Called by the flight loop to send telemetry
/// Non-blocking: always succeeds immediately, overwrites previous data if not sent yet
pub fn update_telemetry(data: &[u8; crate::packet::Packet::SIZE]) {
    unsafe {
        // Copy data to buffer
        (*TELEMETRY_BUF.data.get()).copy_from_slice(data);
        // Mark as ready
        TELEMETRY_BUF.ready.store(true, Ordering::Release);
    }
}

/// Outbound string channel for logs/dumps in release mode
static STRING_OUTBOUND: Channel<CriticalSectionRawMutex, heapless::String<64>, 16> = Channel::new();

/// Sends a string over the umbilical USB connection (release mode only)
pub fn print_str(s: &str) {
    let mut chunk = heapless::String::<64>::new();
    // Split into 64-byte chunks if needed, but for now just send what fits
    let _ = chunk.push_str(&s[..core::cmp::min(s.len(), 64)]);
    let _ = STRING_OUTBOUND.try_send(chunk);
}

/// Called by the flight loop each cycle to poll for incoming umbilical commands.
/// Returns `None` if no command is pending.
pub fn try_recv_command() -> Option<UmbilicalCommand> {
    COMMANDS.try_receive().ok()
}

/// Simulation helper: injects a command into the channel as if it came from USB.
pub fn push_command(cmd: UmbilicalCommand) {
    let _ = COMMANDS.try_send(cmd);
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
    embassy_usb_logger::run!(
        { constants::USB_LOGGER_BUFFER_SIZE },
        log::LevelFilter::Info,
        driver
    );
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
            // Priority 1: Check for logged strings/dumps
            while let Ok(msg) = STRING_OUTBOUND.try_receive() {
                match sender.write_packet(msg.as_bytes()).await {
                    Ok(_) => {}
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => break,
                }
            }

            // Priority 2: Check for telemetry
            if TELEMETRY_BUF.ready.load(Ordering::Acquire) {
                let data = unsafe { *TELEMETRY_BUF.data.get() };
                TELEMETRY_BUF.ready.store(false, Ordering::Release);

                // Build framed packet: [0xAA, 0x55, ...82 bytes telemetry...] = 84 bytes
                let mut frame = [0u8; FRAME_SIZE];
                frame[0..2].copy_from_slice(&SYNC_HEADER);
                frame[2..FRAME_SIZE].copy_from_slice(&data);

                // Send first 64 bytes of frame
                match sender.write_packet(&frame[0..64]).await {
                    Ok(_) => {}
                    Err(EndpointError::BufferOverflow) => panic!("Buffer overflow on first chunk"),
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                };

                // Send remaining 20 bytes of frame
                match sender.write_packet(&frame[64..FRAME_SIZE]).await {
                    Ok(_) => {}
                    Err(EndpointError::BufferOverflow) => panic!("Buffer overflow on second chunk"),
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                };
            }

            Timer::after_millis(50).await; // Faster poll for better responsiveness
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
                b"<G>" => Some(UmbilicalCommand::DumpFlash),
                b"<W>" => Some(UmbilicalCommand::WipeFlash),
                b"<I>" => Some(UmbilicalCommand::FlashInfo),
                b"<1>" => Some(UmbilicalCommand::PayloadN1),
                b"<2>" => Some(UmbilicalCommand::PayloadN2),
                b"<3>" => Some(UmbilicalCommand::PayloadN3),
                b"<4>" => Some(UmbilicalCommand::PayloadN4),
                _ => None,
            };

            if let Some(c) = cmd {
                COMMANDS.try_send(c).ok();
            }

            Timer::after_millis(100).await;
        }
    }
}
