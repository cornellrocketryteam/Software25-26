use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_usb::{UsbDevice, driver::EndpointError};

use crate::module::{self, UsbDriver};
use core::sync::atomic::{AtomicBool, Ordering};

/// Number of comma-separated fields the FSW emits after the `$TELEM,` prefix.
/// Host-side parsers must match this exactly.
pub const TELEM_FIELD_COUNT: usize = 22;

/// Global software umbilical connection tracked by embassy-usb.
static IS_CONNECTED: AtomicBool = AtomicBool::new(false);

/// While true, `emit_telemetry` is a no-op so a long flash/FRAM dump on the
/// shared text channel isn't interleaved with `$TELEM,` lines (which would
/// confuse the host line parser) and doesn't get throttled behind queued
/// telemetry.
static DUMP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Returns whether the ground station umbilical is actively connected via USB.
pub fn is_connected() -> bool {
    IS_CONNECTED.load(Ordering::Relaxed)
}

/// Call before a flash/FRAM dump begins. Suppresses telemetry emission until
/// `end_dump()` is called.
pub fn begin_dump() {
    DUMP_IN_PROGRESS.store(true, Ordering::Release);
}

/// Call after a dump completes (or on error) to resume telemetry emission.
pub fn end_dump() {
    DUMP_IN_PROGRESS.store(false, Ordering::Release);
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
    FaultMode,
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

/// Emit a telemetry line in parseable CSV format.
/// Format: `$TELEM,<flight_mode>,<pressure>,...,<sv_open>,<mav_open>\n`
/// Suppressed while a dump is in progress (see `begin_dump`/`end_dump`).
pub fn emit_telemetry(packet: &crate::packet::Packet) {
    if DUMP_IN_PROGRESS.load(Ordering::Acquire) {
        return;
    }
    let mut buf = [0u8; 512];
    let len = {
        use core::fmt::Write;
        let mut w = BufWriter::new(&mut buf);
        let _ = write!(
            w,
            "$TELEM,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            packet.flight_mode,
            packet.pressure,
            packet.temp,
            packet.altitude,
            packet.latitude,
            packet.longitude,
            packet.num_satellites,
            packet.timestamp,
            packet.mag_x,
            packet.mag_y,
            packet.mag_z,
            packet.accel_x,
            packet.accel_y,
            packet.accel_z,
            packet.gyro_x,
            packet.gyro_y,
            packet.gyro_z,
            packet.pt3,
            packet.pt4,
            packet.rtd,
            packet.sv_open as u8,
            packet.mav_open as u8,
        );
        w.offset
    };
    send_bytes(&buf[..len]);
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
/// In release builds the logger is compiled out so the wire carries only
/// telemetry + explicit `print_str`/`print_bytes` output.
pub fn setup(spawner: &Spawner, usb_driver: UsbDriver) {
    let (usb_device, usb_class) = module::init_usb_device(usb_driver);
    let (sender, receiver) = usb_class.split();

    #[cfg(debug_assertions)]
    init_logger();

    spawner.spawn(usb_task(usb_device).unwrap());
    spawner.spawn(usb_sender_task(sender).unwrap());
    spawner.spawn(usb_receiver_task(receiver).unwrap());
}

// ============================================================================
// USB Serial Logger (replaces embassy-usb-logger) — debug builds only
// ============================================================================

#[cfg(debug_assertions)]
struct UsbSerialLogger;

#[cfg(debug_assertions)]
static LOGGER: UsbSerialLogger = UsbSerialLogger;

#[cfg(debug_assertions)]
fn init_logger() {
    unsafe {
        let _ = log::set_logger_racy(&LOGGER);
    }
    log::set_max_level(log::LevelFilter::Info);
}

#[cfg(debug_assertions)]
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

/// Reads text chunks from the outbound channel and writes them to the USB sender.
#[embassy_executor::task]
async fn usb_sender_task(mut sender: Sender<'static, UsbDriver>) -> ! {
    loop {
        sender.wait_connection().await;
        IS_CONNECTED.store(true, Ordering::Relaxed);

        loop {
            let msg = RAW_OUTBOUND.receive().await;
            match sender.write_packet(&msg).await {
                Ok(_) => {}
                Err(EndpointError::Disabled) => {
                    IS_CONNECTED.store(false, Ordering::Relaxed);
                    break;
                }
                Err(_) => break,
            }
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
                b"<X>" => Some(UmbilicalCommand::FaultMode),
                _ => None,
            };

            if let Some(c) = cmd {
                COMMANDS.try_send(c).ok();
            }

            Timer::after_millis(100).await;
        }
    }
}
