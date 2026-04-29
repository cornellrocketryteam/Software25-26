use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use embassy_usb::{UsbDevice, driver::EndpointError};

use crate::constants::HEARTBEAT_TIMEOUT_MS;
use crate::module::{self, UsbDriver};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Number of comma-separated fields the FSW emits after the `$TELEM,` prefix.
/// Host-side parsers must match this exactly.
pub const TELEM_FIELD_COUNT: usize = 57;

/// Whether any heartbeat has ever been received. Separates the "never seen"
/// state from the wrapping `LAST_HEARTBEAT_MS` value (RP2040 lacks AtomicU64,
/// so we truncate millis to u32 and rely on wrapping subtraction for freshness
/// — valid for diffs well under 2^31 ms).
static HEARTBEAT_EVER: AtomicBool = AtomicBool::new(false);
static LAST_HEARTBEAT_MS: AtomicU32 = AtomicU32::new(0);

/// While true, `emit_telemetry` is a no-op so a long flash/FRAM dump on the
/// shared text channel isn't interleaved with `$TELEM,` lines (which would
/// confuse the host line parser) and doesn't get throttled behind queued
/// telemetry.
static DUMP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Returns whether the ground station umbilical is actively connected.
/// True iff a heartbeat (`<H>`) was received within the last
/// `HEARTBEAT_TIMEOUT_MS`. USB enumeration state is intentionally ignored —
/// it lies when the host process dies but the cable stays plugged.
pub fn is_connected() -> bool {
    if !HEARTBEAT_EVER.load(Ordering::Relaxed) {
        return false;
    }
    let last = LAST_HEARTBEAT_MS.load(Ordering::Relaxed);
    let now = Instant::now().as_millis() as u32;
    now.wrapping_sub(last) < HEARTBEAT_TIMEOUT_MS as u32
}

/// Records a heartbeat from the ground station. Called when `<H>` is received.
fn record_heartbeat() {
    LAST_HEARTBEAT_MS.store(Instant::now().as_millis() as u32, Ordering::Relaxed);
    HEARTBEAT_EVER.store(true, Ordering::Relaxed);
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
    PayloadA1,
    PayloadA2,
    PayloadA3,
    WipeFramReboot,
    KeyArm,
    KeyDisarm,
    SetBlimsTarget { lat: f32, lon: f32 },
    TriggerDrogue, // Remove this functionality for real code
    TriggerMain,   // Remove this functionality for real code
    //DrogueMode,    // Remove this functionality for real code
    //MainMode,      // Remove this functionality for real code
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
    let mut buf = [0u8; 1024];
    let len = {
        use core::fmt::Write;
        let mut w = BufWriter::new(&mut buf);
        let _ = write!(
            w,
            "$TELEM,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
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
            packet.ssa_drogue_deployed,
            packet.ssa_main_deployed,
            packet.cmd_n1,
            packet.cmd_n2,
            packet.cmd_n3,
            packet.cmd_n4,
            packet.cmd_a1,
            packet.cmd_a2,
            packet.cmd_a3,
            packet.airbrake_state,
            packet.predicted_apogee,
            packet.h_acc,
            packet.v_acc,
            packet.vel_n,
            packet.vel_e,
            packet.vel_d,
            packet.g_speed,
            packet.s_acc,
            packet.head_acc,
            packet.fix_type,
            packet.head_mot,
            packet.blims_motor_position,
            packet.blims_phase_id,
            packet.blims_pid_p,
            packet.blims_pid_i,
            packet.blims_bearing,
            packet.blims_loiter_step,
            packet.blims_heading_des,
            packet.blims_heading_error,
            packet.blims_error_integral,
            packet.blims_dist_to_target_m,
            packet.blims_target_lat,
            packet.blims_target_lon,
            packet.blims_wind_from_deg,
            packet.ms_since_boot_cfc,
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
        // Never inject log text into the binary stream during a flash dump.
        if DUMP_IN_PROGRESS.load(Ordering::Acquire) {
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

        loop {
            let msg = RAW_OUTBOUND.receive().await;
            match sender.write_packet(&msg).await {
                Ok(_) => {}
                Err(EndpointError::Disabled) => break,
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

        loop {
            let n = match receiver.read_packet(&mut buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => {
                    log::warn!("USB RX: buffer overflow, dropping packet");
                    continue;
                }
                Err(EndpointError::Disabled) => {
                    break;
                }
            };

            let data = &buf[..n];

            // Heartbeat is hot-path: bump the timestamp and skip the command channel.
            if data == b"<H>" {
                record_heartbeat();
                continue;
            }

            // Variable-length: BLiMS target set, format `<T,<lat>,<lon>>`.
            if data.len() >= 4 && &data[..3] == b"<T," && data[data.len() - 1] == b'>' {
                let body = &data[3..data.len() - 1];
                let parsed = core::str::from_utf8(body).ok().and_then(|s| {
                    let mut parts = s.split(',');
                    let lat = parts.next()?.parse::<f32>().ok()?;
                    let lon = parts.next()?.parse::<f32>().ok()?;
                    if parts.next().is_some() {
                        return None;
                    }
                    Some((lat, lon))
                });
                match parsed {
                    Some((lat, lon))
                        if (-90.0..=90.0).contains(&lat)
                            && (-180.0..=180.0).contains(&lon) =>
                    {
                        COMMANDS
                            .try_send(UmbilicalCommand::SetBlimsTarget { lat, lon })
                            .ok();
                    }
                    Some((lat, lon)) => {
                        log::warn!(
                            "Umbilical SetBlimsTarget rejected: out of range lat={} lon={}",
                            lat, lon
                        );
                    }
                    None => {
                        log::warn!("Umbilical SetBlimsTarget parse failed");
                    }
                }
                continue;
            }

            let cmd = match data {
                b"<L>" => Some(UmbilicalCommand::Launch),
                b"<M>" => Some(UmbilicalCommand::OpenMav),
                b"<m>" => Some(UmbilicalCommand::CloseMav),
                b"<S>" => Some(UmbilicalCommand::OpenSv),
                b"<s>" => Some(UmbilicalCommand::CloseSv),
                b"<V>" => Some(UmbilicalCommand::Safe),
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
                b"<X>" => Some(UmbilicalCommand::WipeFramReboot),
                b"<A1>" => Some(UmbilicalCommand::PayloadA1),
                b"<A2>" => Some(UmbilicalCommand::PayloadA2),
                b"<A3>" => Some(UmbilicalCommand::PayloadA3),
                b"<KA>" => Some(UmbilicalCommand::KeyArm),
                b"<KD>" => Some(UmbilicalCommand::KeyDisarm),
                b"<D>" => Some(UmbilicalCommand::TriggerDrogue),
                b"<d>" => Some(UmbilicalCommand::TriggerMain),
                // b"<DR>" => Some(UmbilicalCommand::DrogueMode),
                // b"<MR>" => Some(UmbilicalCommand::MainMode),
                _ => None,
            };

            if let Some(c) = cmd {
                COMMANDS.try_send(c).ok();
            }

            Timer::after_millis(100).await;
        }
    }
}
