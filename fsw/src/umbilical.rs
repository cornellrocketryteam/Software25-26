use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_usb::{UsbDevice, driver::EndpointError};

use crate::module::{self, UsbDriver};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// Sync header prepended to every binary telemetry frame so the receiver can
/// locate packet boundaries in the byte stream. Total frame = 2 + 82 = 84 bytes.
pub const SYNC_HEADER: [u8; 2] = [0xAA, 0x55];
const FRAME_SIZE: usize = SYNC_HEADER.len() + crate::packet::Packet::SIZE;

/// Global software umbilical connection tracked by embassy-usb.
static IS_CONNECTED: AtomicBool = AtomicBool::new(false);

/// Single-slot atomic telemetry buffer. Flight loop writes, sender task reads;
/// both non-blocking. Overwritten in place if the sender hasn't drained the
/// previous frame yet.
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
    DumpFram,
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

/// Outbound text channel for logs and telemetry
static RAW_OUTBOUND: Channel<CriticalSectionRawMutex, heapless::Vec<u8, 64>, 32> = Channel::new();

/// Sends raw bytes over the USB connection, chunked to 64-byte packets
fn send_bytes(data: &[u8]) {
    let mut offset = 0;
    while offset < data.len() {
        let mut chunk = heapless::Vec::<u8, 64>::new();
        let len = core::cmp::min(data.len() - offset, 64);
        let _ = chunk.extend_from_slice(&data[offset..offset + len]);
        if RAW_OUTBOUND.try_send(chunk).is_err() {
            break; // Channel full, drop
        }
        offset += len;
    }
}

/// Sends a string over the USB connection (used by flash dump/status)
pub fn print_str(s: &str) {
    send_bytes(s.as_bytes());
}

/// Sends raw bytes over the USB connection (used by flash dump)
pub fn print_bytes(data: &[u8]) {
    send_bytes(data);
}

/// Sends raw bytes over USB, awaiting channel space instead of dropping.
/// Use this for flash dumps where every byte must be delivered.
pub async fn print_bytes_async(data: &[u8]) {
    let mut offset = 0;
    while offset < data.len() {
        let mut chunk = heapless::Vec::<u8, 64>::new();
        let len = core::cmp::min(data.len() - offset, 64);
        let _ = chunk.extend_from_slice(&data[offset..offset + len]);
        RAW_OUTBOUND.send(chunk).await; // blocks until channel has space
        offset += len;
    }
}

/// Publish a telemetry packet for the sender task. Non-blocking: writes the
/// 82-byte LE serialization into the atomic buffer and marks it ready,
/// overwriting any prior frame the sender hasn't drained yet.
pub fn emit_telemetry(packet: &crate::packet::Packet) {
    let bytes = packet.to_bytes();
    unsafe {
        (*TELEMETRY_BUF.data.get()).copy_from_slice(&bytes);
    }
    TELEMETRY_BUF.ready.store(true, Ordering::Release);
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

/// Initialize USB subsystem: CdcAcmClass for bidirectional text communication.
/// Logs go out as text (readable in any serial monitor), commands come in as `<X>` tokens.
pub fn setup(spawner: &Spawner, usb_driver: UsbDriver) {
    let (usb_device, usb_class) = module::init_usb_device(usb_driver);
    let (sender, receiver) = usb_class.split();

    // Register our USB serial logger as the global `log` implementation
    init_logger();

    spawner.spawn(usb_task(usb_device).unwrap());
    spawner.spawn(usb_sender_task(sender).unwrap());
    spawner.spawn(usb_receiver_task(receiver).unwrap());
}

// ============================================================================
// USB Serial Logger (replaces embassy-usb-logger)
// ============================================================================

struct UsbSerialLogger;

static LOGGER: UsbSerialLogger = UsbSerialLogger;

fn init_logger() {
    unsafe {
        let _ = log::set_logger_racy(&LOGGER);
    }
    log::set_max_level(log::LevelFilter::Info);
}

impl log::Log for UsbSerialLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut buf = [0u8; 256];
        let len = {
            use core::fmt::Write;
            let mut w = BufWriter::new(&mut buf);
            let _ = write!(w, "[{}] {}\n", record.level(), record.args());
            w.offset
        };
        send_bytes(&buf[..len]);
    }

    fn flush(&self) {}
}

// ============================================================================
// Helper: fixed-size buffer writer for no_std formatting
// ============================================================================

struct BufWriter<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> BufWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }
}

impl<'a> core::fmt::Write for BufWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.offset;
        let len = core::cmp::min(bytes.len(), remaining);
        self.buf[self.offset..self.offset + len].copy_from_slice(&bytes[..len]);
        self.offset += len;
        if len < bytes.len() {
            Err(core::fmt::Error)
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// USB Tasks
// ============================================================================

#[embassy_executor::task]
async fn usb_task(mut usb_device: UsbDevice<'static, UsbDriver>) -> ! {
    usb_device.run().await
}

/// Drains two sources onto the USB sender:
///   1. `RAW_OUTBOUND` — text chunks from logs, FRAM dump, flash dump.
///   2. `TELEMETRY_BUF` — latest binary telemetry frame, sent as
///      [SYNC_HEADER || 82-byte LE packet] in two USB packets (64 + 20).
/// Text is given priority so logs/dumps don't starve behind telemetry.
#[embassy_executor::task]
async fn usb_sender_task(mut sender: Sender<'static, UsbDriver>) -> ! {
    loop {
        sender.wait_connection().await;
        IS_CONNECTED.store(true, Ordering::Relaxed);

        loop {
            // Priority 1: drain any queued text bytes.
            while let Ok(msg) = RAW_OUTBOUND.try_receive() {
                match sender.write_packet(&msg).await {
                    Ok(_) => {}
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => break,
                }
            }

            // Priority 2: send a telemetry frame if one is ready.
            if TELEMETRY_BUF.ready.load(Ordering::Acquire) {
                let data = unsafe { *TELEMETRY_BUF.data.get() };
                TELEMETRY_BUF.ready.store(false, Ordering::Release);

                let mut frame = [0u8; FRAME_SIZE];
                frame[0..2].copy_from_slice(&SYNC_HEADER);
                frame[2..FRAME_SIZE].copy_from_slice(&data);

                match sender.write_packet(&frame[0..64]).await {
                    Ok(_) => {}
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => break,
                }
                match sender.write_packet(&frame[64..FRAME_SIZE]).await {
                    Ok(_) => {}
                    Err(EndpointError::Disabled) => {
                        IS_CONNECTED.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => break,
                }
            }

            Timer::after_millis(50).await;
        }
    }
}

/// Reads USB packets from the host and parses command tokens like `<L>`, `<M>`, etc.
#[embassy_executor::task]
async fn usb_receiver_task(mut receiver: Receiver<'static, UsbDriver>) -> ! {
    let mut buf = [0; 64];
    loop {
        receiver.wait_connection().await;
        IS_CONNECTED.store(true, Ordering::Relaxed);

        loop {
            let n = match receiver.read_packet(&mut buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => {
                    log::warn!("USB RX: buffer overflow, dropping packet");
                    continue;
                }
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
                b"<f>" => Some(UmbilicalCommand::DumpFram),
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