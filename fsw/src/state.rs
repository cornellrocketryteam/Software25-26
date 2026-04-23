use crate::constants;
use crate::module::*;

use crate::packet::Packet;

use crate::driver::bmp390::Bmp390Sensor;
use crate::driver::lsm6dsox::Lsm6dsoxSensor;
use crate::driver::rfd900x::Rfd900x;
use crate::driver::ublox_max_m10s::UbloxMaxM10s;
use crate::driver::ads1015::Ads1015Sensor;
use crate::driver::onboard_flash::OnboardFlash;

use embassy_rp::gpio::{Input, Output};
use embassy_rp::uart::{Async, Uart, UartTx};
use embassy_time::{Duration, Instant, with_timeout};

use crate::actuator::{Ssa, Buzzer, Mav, SV, Chute, AirbrakeActuator};

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SensorState {
    //OFF = 0,
    VALID = 1,
    INVALID = 2,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FlightMode {
    Startup = 0,
    Standby = 1,
    Ascent = 2,
    Coast = 3,
    DrogueDeployed = 4,
    MainDeployed = 5,
    Fault = 6,
}

impl FlightMode {
    pub fn from_u32(raw: u32) -> Self {
        match raw {
            0 => Self::Startup,
            1 => Self::Standby,
            2 => Self::Ascent,
            3 => Self::Coast,
            4 => Self::DrogueDeployed,
            5 => Self::MainDeployed,
            _ => Self::Fault,
        }
    }
}

pub struct FlightState {
    // packet
    pub packet: Packet,
    // state variables
    pub flight_mode: FlightMode,
    pub cycle_count: u32,
    pub key_armed: bool,
    pub umbilical_connected: bool,

    // altimeter
    altimeter: Bmp390Sensor<'static>,
    pub altimeter_state: SensorState,
    pub reference_pressure: f32,

    // gps
    gps: UbloxMaxM10s<'static, I2cDevice<'static>>,
    gps_ok: bool,

    // imu
    imu: Lsm6dsoxSensor,

    // adc
    adc: Ads1015Sensor,

    // actuators
    arming_switch: Input<'static>,
    cfc_arm: Input<'static>,
    pub cfc_arm_active: bool,
    pub arming_altitude: f32,

    pub ssa: Ssa<'static>,
    pub buzzer: Buzzer<'static>,
    pub mav: Mav<'static>,
    pub sv: SV<'static>,
    pub airbrake_system: AirbrakeActuator<'static>,

    // telemetry
    radio: Rfd900x<'static>,

    // comms
    pub payload_comms_ok: bool,
    pub recovery_comms_ok: bool,

    // QSPI Flash
    flash: OnboardFlash<'static>,

    // Snapshot-ring throttle (replaces FRAM periodic logging at 1 Hz)
    last_snapshot_log: Instant,

    // External Comms
    pub payload_uart: UartTx<'static, Async>,

    #[cfg(feature = "sim_payload")]
    pub sim_radio_command: Option<crate::packet::Command>,
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi_bus: &'static SharedSpi,
        altimeter_cs: Output<'static>,
        arming_switch: Input<'static>,
        cfc_arm: Input<'static>,
        uart: Uart<'static, Async>,
        ssa: Ssa<'static>,
        buzzer: Buzzer<'static>,
        mav: Mav<'static>,
        sv: SV<'static>,
        airbrake_system: AirbrakeActuator<'static>,
        mut flash: OnboardFlash<'static>,
        payload_uart: UartTx<'static, Async>,
    ) -> Self {
        let mut packet = Packet::default();
        let init_to = Duration::from_millis(constants::SENSOR_INIT_TIMEOUT_MS);
        let flash_to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);

        // Initialize flash FIRST on a clean SPI/DMA bus. BMP390 init runs many
        // DMA transactions (calibration reads, soft reset, config writes) that
        // can leave the SPI/DMA engine in a state where 256-byte page reads hang.
        // Running the flash binary-search scan before BMP390 avoids this.
        let flash_ok = match with_timeout(
            Duration::from_millis(constants::SENSOR_INIT_TIMEOUT_MS),
            flash.initialize_logging(),
        ).await {
            Ok(Ok(_)) => {
                log::info!("QSPI Flash logging initialized.");
                true
            }
            Ok(Err(e)) => {
                log::error!("Failed to initialize QSPI Flash logging: {:?}", e);
                false
            }
            Err(_) => {
                log::error!("QSPI Flash init TIMEOUT");
                false
            }
        };
        flash.flash_ok = flash_ok;

        // Initialize the snapshot ring (replaces FRAM for crash recovery).
        let mut stored_mode = FlightMode::Startup;
        let mut stored_cycle_count = 0u32;
        if flash_ok {
            match with_timeout(flash_to, flash.initialize_snapshot_ring()).await {
                Ok(Ok(_)) => match with_timeout(flash_to, flash.read_latest_snapshot()).await {
                    Ok(Ok(Some(snap))) => {
                        stored_mode = FlightMode::from_u32(snap.flight_mode);
                        stored_cycle_count = snap.cycle_count;
                        log::info!(
                            "Snapshot recovered: mode={:?} cycle={} alt={:.2}",
                            stored_mode, stored_cycle_count, snap.altitude
                        );
                    }
                    Ok(Ok(None)) => log::info!("Snapshot ring empty — starting fresh."),
                    Ok(Err(e)) => log::warn!("Snapshot read failed: {:?}", e),
                    Err(_) => log::warn!("Snapshot read TIMEOUT"),
                },
                Ok(Err(e)) => log::warn!("Snapshot ring init failed: {:?}", e),
                Err(_) => log::warn!("Snapshot ring init TIMEOUT"),
            }
        }

        // Attempt to read the last packet state from Onboard QSPI Flash
        if flash_ok {
            match with_timeout(flash_to, flash.read_packet()).await {
                Ok(Ok(recovered_packet)) => {
                    if recovered_packet.flight_mode <= (FlightMode::Fault as u32) {
                        log::info!("Successfully recovered previous packet from QSPI Flash.");
                        packet = recovered_packet;
                    } else {
                        log::info!("QSPI Flash data appears uninitialized or invalid.");
                    }
                }
                Ok(Err(_)) => {
                    log::warn!("Failed to recover packet from QSPI Flash.");
                }
                Err(_) => {
                    log::warn!("QSPI Flash recover-packet TIMEOUT");
                }
            }
        }

        log::info!("STATE: Initializing altimeter (BMP390)...");
        let altimeter = match with_timeout(init_to, Bmp390Sensor::new(spi_bus, altimeter_cs)).await {
            Ok(s) => s,
            Err(_) => {
                log::error!("STATE: BMP390 init TIMEOUT — marking unavailable");
                Bmp390Sensor::unavailable()
            }
        };
        let altimeter_init = if altimeter.is_init() {
            log::info!("STATE: Altimeter OK");
            SensorState::VALID
        } else {
            log::error!("STATE: Altimeter FAILED — flight will fault");
            SensorState::INVALID
        };

        log::info!("STATE: Initializing GPS (uBlox MAX-M10S)...");
        let mut gps = UbloxMaxM10s::new(i2c_bus);

        // Configure GPS module to output NAV-PVT messages
        let gps_ok = match with_timeout(init_to, gps.configure()).await {
            Ok(Ok(_)) => { log::info!("STATE: GPS configured OK"); true }
            Ok(Err(e)) => {
                log::error!("STATE: GPS configure FAILED: {:?}", e);
                false
            }
            Err(_) => {
                log::error!("STATE: GPS configure TIMEOUT — I²C bus may be locked");
                false
            }
        };

        // Always attempt IMU/ADC init regardless of GPS result. Each is wrapped
        // in a timeout so a locked I2C bus (SDA stuck low) at worst adds 500ms
        // per sensor — it will not hang. GPS failure does not affect flight logic.
        log::info!("STATE: Initializing IMU (LSM6DSOX)...");
        let imu = match with_timeout(init_to, Lsm6dsoxSensor::new(i2c_bus)).await {
            Ok(s) => s,
            Err(_) => {
                log::error!("STATE: IMU init TIMEOUT — marking unavailable");
                Lsm6dsoxSensor::unavailable(i2c_bus)
            }
        };
        log::info!("STATE: Initializing ADC (ADS1015)...");
        let adc = match with_timeout(init_to, Ads1015Sensor::new(i2c_bus)).await {
            Ok(s) => s,
            Err(_) => {
                log::error!("STATE: ADC init TIMEOUT — marking unavailable");
                Ads1015Sensor::unavailable(i2c_bus)
            }
        };
        log::info!("STATE: IMU and ADC init complete");
        log::info!("STATE: Initializing radio (RFD900x)...");
        let radio = Rfd900x::new(uart);
        log::info!("STATE: Radio ready");

        Self {
            packet: packet,
            flight_mode: stored_mode,
            cycle_count: stored_cycle_count,
            key_armed: false,
            umbilical_connected: false,
            altimeter: altimeter,
            altimeter_state: altimeter_init,
            gps: gps,
            gps_ok,
            imu: imu,
            adc: adc,
            arming_switch: arming_switch,
            cfc_arm: cfc_arm,
            cfc_arm_active: false,
            arming_altitude: 0.0,
            radio: radio,
            reference_pressure: 0.0,
            payload_comms_ok: true,
            recovery_comms_ok: true,
            ssa,
            buzzer,
            mav,
            sv,
            airbrake_system,
            flash,
            last_snapshot_log: Instant::now(),
            payload_uart,

            #[cfg(feature = "sim_payload")]
            sim_radio_command: None,
        }
    }

    pub fn read_altimeter(&mut self) -> f32{
        return self.packet.altitude;
    }

    pub fn read_barometer(&mut self) -> f32{
        return self.packet.pressure;
    }

    pub async fn update_actuators(&mut self) {
        self.ssa.update();
        self.buzzer.update();
        self.mav.update();
        self.sv.update();

        // Sync actuator state into telemetry packet
        self.packet.sv_open = self.sv.is_open();
        self.packet.mav_open = self.mav.is_open();
    }

    // Actuator wrappers with FRAM writing

    pub async fn trigger_drogue(&mut self) {
        log::info!("ACTUATOR: Triggering Drogue");
        self.ssa.trigger(Chute::Drogue, crate::constants::SSA_THRESHOLD_MS);
    }

    pub async fn trigger_main(&mut self) {
        log::info!("ACTUATOR: Triggering Main");
        self.ssa.trigger(Chute::Main, crate::constants::SSA_THRESHOLD_MS);
    }

    pub fn buzz(&mut self, num: u32) {
        log::info!("ACTUATOR: Buzzing {} times", num);
        self.buzzer.buzz(num);
    }
    pub async fn open_mav(&mut self, duration: u64) {
        log::info!("ACTUATOR: Opening MAV");
        self.mav.open(duration);
    }

    pub async fn close_mav(&mut self) {
        log::info!("ACTUATOR: Closing MAV");
        self.mav.close();
    }

    pub async fn open_sv(&mut self, duration: u64) {
        log::info!("ACTUATOR: Opening SV");
        self.sv.open(duration);
    }

    pub async fn close_sv(&mut self) {
         log::info!("ACTUATOR: Closing SV");
         self.sv.close();
    }

    pub async fn read_sensors(&mut self) {
        self.update_actuators().await;

        // Update packet flight mode
        self.packet.flight_mode = self.flight_mode as u32;

        // Update key armed status
        self.key_armed = self.arming_switch.is_high();
        self.umbilical_connected = crate::umbilical::is_connected();
        self.cfc_arm_active = self.cfc_arm.is_high();

        let read_to = Duration::from_millis(constants::SENSOR_READ_TIMEOUT_MS);

        // Read altimeter and update packet
        match with_timeout(read_to, self.altimeter.read_into_packet(&mut self.packet)).await {
            Ok(Ok(_)) => {
                self.altimeter_state = SensorState::VALID;
                log::info!(
                    "BMP | Pressure = {:.2} Pa, Temp = {:.2} °C, Alt = {:.2} m",
                    self.packet.pressure,
                    self.packet.temp,
                    self.packet.altitude
                );
            }
            Ok(Err(e)) => {
                self.altimeter_state = SensorState::INVALID;
                log::error!("Failed to read BMP390: {:?}", e);
            }
            Err(_) => {
                self.altimeter_state = SensorState::INVALID;
                log::error!("BMP390 read TIMEOUT");
            }
        }

        // Read GPS and update packet
        if self.gps_ok {
            match with_timeout(read_to, self.gps.read_into_packet(&mut self.packet)).await {
                Ok(Ok(_)) => {
                    log::info!(
                        "GPS | Lat = {:.6}°, Lon = {:.6}°, Sats = {}, Time = {:.0} s",
                        self.packet.latitude,
                        self.packet.longitude,
                        self.packet.num_satellites,
                        self.packet.timestamp
                    );
                }
                Ok(Err(e)) => {
                    log::error!("Failed to read GPS: {:?}", e);
                }
                Err(_) => {
                    log::error!("GPS read TIMEOUT — I2C bus may be locked");
                }
            }
        }

        // Read IMU and update packet
        match with_timeout(read_to, self.imu.read_into_packet(&mut self.packet)).await {
            Ok(Ok(_)) => {
                log::info!(
                    "IMU | Accel: X={:.2} Y={:.2} Z={:.2} m/s² | Gyro: X={:.2} Y={:.2} Z={:.2} °/s",
                    self.packet.accel_x,
                    self.packet.accel_y,
                    self.packet.accel_z,
                    self.packet.gyro_x,
                    self.packet.gyro_y,
                    self.packet.gyro_z
                );
            }
            Ok(Err(e)) => {
                log::error!("Failed to read LSM6DSOX IMU: {:?}", e);
            }
            Err(_) => {
                log::error!("LSM6DSOX IMU read TIMEOUT");
            }
        }

        // Read ADC and update packet
        match with_timeout(read_to, self.adc.read_into_packet(&mut self.packet)).await {
            Ok(Ok(_)) => {
                log::info!(
                    "ADC | PT3={:.0} PT4={:.0} RTD={:.0} (raw)",
                    self.packet.pt3,
                    self.packet.pt4,
                    self.packet.rtd
                );
            }
            Ok(Err(e)) => {
                log::error!("Failed to read ADS1015 ADC: {:?}", e);
            }
            Err(_) => {
                log::error!("ADS1015 ADC read TIMEOUT");
            }
        }

        log::info!("Flight mode: {:?}\n", self.flight_mode);
    }

    pub async fn transmit(&mut self) {
        let data = self.packet.to_bytes();

        match self.radio.send(&data).await {
            Ok(_) => {
                log::info!("RFD | Data transmitted successfully!");
            }
            Err(e) => {
                log::warn!("RFD | Failed to transmit packet via radio: {:?}", e);
            }
        }

        // Emit telemetry as a parseable text line over USB
        crate::umbilical::emit_telemetry(&self.packet);
    }

    pub async fn receive_radio(&mut self, buffer: &mut [u8]) -> Result<(), embassy_rp::uart::Error> {
        let result = self.radio.receive_packet(buffer).await;
        if result.is_ok() {
            log::info!("RFD | Packet received successfully!");
        }
        result
    }

    /// Receive and decode a full telemetry packet
    pub async fn receive_telemetry(&mut self) -> Result<Packet, embassy_rp::uart::Error> {
        let mut buf = [0u8; Packet::SIZE];
        self.radio.receive_packet(&mut buf).await?;
        let packet = Packet::from_bytes(&buf);
        log::info!("RFD | Telemetry packet decoded successfully!");
        Ok(packet)
    }

    pub async fn poll_radio_command(&mut self) -> Option<crate::packet::Command> {
        #[cfg(feature = "sim_payload")]
        if let Some(cmd) = self.sim_radio_command.take() {
            return Some(cmd);
        }

        let mut buf = [0u8; 32];
        // Short timeout read to check for commands without blocking the loop
        // We use the basic receive here as commands might not have the sync-word
        // unless they are sent by another FSW board. 
        if let Ok(Ok(_)) = embassy_time::with_timeout(
            embassy_time::Duration::from_millis(10),
            self.radio.receive(&mut buf),
        )
        .await
        {
            // Simple string-based command parsing
            if buf.starts_with(b"VNT") {
                return Some(crate::packet::Command::Vent);
            } else if buf.starts_with(b"N1") {
                return Some(crate::packet::Command::N1);
            } else if buf.starts_with(b"N2") {
                return Some(crate::packet::Command::N2);
            } else if buf.starts_with(b"N3") {
                return Some(crate::packet::Command::N3);
            } else if buf.starts_with(b"N4") {
                return Some(crate::packet::Command::N4);
            } else if buf.starts_with(b"FM") {
                // Example: "FM2" for Coast
                if let Some(digit) = (buf[2] as char).to_digit(10) {
                    return Some(crate::packet::Command::ForceMode(digit as u32));
                }
            }
        }
        None
    }

    // Appends the current packet as CSV to the onboard QSPI Flash memory
    pub async fn save_packet_to_flash(&mut self) {
        if !self.flash.flash_ok {
            return;
        }
        let to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);
        match with_timeout(to, self.flash.append_packet_csv(&self.packet)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => log::warn!("Failed to append packet CSV to QSPI Flash: {:?}", e),
            Err(_) => log::warn!("QSPI Flash append TIMEOUT"),
        }
    }

    /// Reads the packet currently stored in the onboard QSPI Flash
    pub async fn read_flash_packet(&mut self) -> Result<Packet, crate::driver::onboard_flash::Error> {
        self.flash.read_packet().await
    }

    pub fn flight_mode_name(&mut self) -> &'static str {
        match self.flight_mode {
            FlightMode::Startup => "Startup",
            FlightMode::Standby => "Standby",
            FlightMode::Ascent => "Ascent",
            FlightMode::Coast => "Coast",
            FlightMode::DrogueDeployed => "DrogueDeployed",
            FlightMode::MainDeployed => "MainDeployed",
            FlightMode::Fault => "Fault",
        }
    }

    /// Periodic snapshot, throttled to 1 Hz. Replaces FRAM periodic logging.
    /// Flash can't absorb 100 Hz writes — erase stalls would block the loop.
    pub async fn log_to_fram(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_snapshot_log) < Duration::from_millis(1000) {
            return;
        }
        self.last_snapshot_log = now;
        self.write_packet_to_fram().await;
    }

    /// Dump the latest snapshot over the umbilical.
    pub async fn dump_fram(&mut self) {
        if !self.flash.flash_ok {
            crate::umbilical::print_str("Snapshot: flash not available\n");
            return;
        }
        let to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);
        match with_timeout(to, self.flash.read_latest_snapshot()).await {
            Ok(Ok(Some(s))) => {
                let mut msg = heapless::String::<160>::new();
                let _ = core::fmt::write(
                    &mut msg,
                    format_args!(
                        "SNAP seq={} mode={} cyc={} p={:.1} t={:.2} alt={:.2} mav={} sv={} pt3={:.1} pt4={:.1} rtd={:.1}\n",
                        s.seq, s.flight_mode, s.cycle_count,
                        s.pressure, s.temp, s.altitude,
                        s.mav_open, s.sv_open, s.pt3, s.pt4, s.rtd
                    ),
                );
                crate::umbilical::print_str(msg.as_str());
            }
            Ok(Ok(None)) => crate::umbilical::print_str("Snapshot: ring empty\n"),
            Ok(Err(e)) => log::warn!("Snapshot dump read failed: {:?}", e),
            Err(_) => log::warn!("Snapshot dump TIMEOUT"),
        }
    }

    /// Erase the snapshot ring.
    pub async fn reset_fram(&mut self) {
        if !self.flash.flash_ok {
            return;
        }
        let to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);
        match with_timeout(to, self.flash.reset_snapshot_ring()).await {
            Ok(Ok(_)) => log::info!("Snapshot ring reset."),
            Ok(Err(e)) => log::warn!("Snapshot reset failed: {:?}", e),
            Err(_) => log::warn!("Snapshot reset TIMEOUT"),
        }
    }

    // Force FlightMode to Standby
    pub async fn trigger_standby(&mut self) {
        self.flight_mode = FlightMode::Standby;
        self.write_packet_to_fram().await;
    }

    /// Append the current packet/state to the snapshot ring.
    pub async fn write_packet_to_fram(&mut self) {
        if !self.flash.flash_ok {
            return;
        }
        let mut snap = crate::driver::onboard_flash::Snapshot {
            seq: 0,
            flight_mode: self.flight_mode as u32,
            cycle_count: self.cycle_count,
            pressure: self.packet.pressure,
            temp: self.packet.temp,
            altitude: self.packet.altitude,
            mav_open: self.packet.mav_open as u32,
            sv_open: self.packet.sv_open as u32,
            pt3: self.packet.pt3,
            pt4: self.packet.pt4,
            rtd: self.packet.rtd,
        };
        let to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);
        match with_timeout(to, self.flash.write_snapshot(&mut snap)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => log::warn!("Snapshot write failed: {:?}", e),
            Err(_) => log::warn!("Snapshot write TIMEOUT"),
        }
    }

    /// Reads all stored CSV data from flash and prints it to the log
    pub async fn print_flash_dump(&mut self) {
        log::info!("--- BEGIN FLASH CSV DUMP ---");
        // Suppress telemetry while the dump is on the wire so $TELEM lines
        // can't interleave with raw flash bytes and so telemetry doesn't
        // back-pressure the dump.
        crate::umbilical::begin_dump();
        crate::umbilical::print_str("--- BEGIN FLASH CSV DUMP ---\n");
        let start = self.flash.get_storage_offset();
        let end = self.flash.get_write_offset();
        let mut offset = start;
        let mut buffer = [0u8; 256];
        let flash_to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);

        while offset < end {
            let chunk_size = core::cmp::min(256, (end - offset) as usize);
            match with_timeout(flash_to, self.flash.read(offset, &mut buffer[..chunk_size])).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    log::error!("Flash read error during dump: {:?}", e);
                    break;
                }
                Err(_) => {
                    log::error!("Flash read TIMEOUT during dump");
                    break;
                }
            }

            // Async send — back-pressures to USB speed so no data is dropped
            crate::umbilical::print_bytes_async(&buffer[..chunk_size]).await;
            // Keep the watchdog fed; a full-flash dump at USB-CDC speeds can
            // easily exceed the flight loop timeout.
            crate::watchdog::feed();

            offset += chunk_size as u32;
        }
        log::info!("--- END FLASH CSV DUMP ---");
        crate::umbilical::print_str("--- END FLASH CSV DUMP ---\n");
        crate::umbilical::end_dump();
    }

    /// Erases all stored CSV data in the flash storage region
    pub async fn wipe_flash_storage(&mut self) {
        log::info!("Wiping QSPI Flash storage...");
        crate::umbilical::print_str("Wiping QSPI Flash... Please wait.\n");
        // Wiping a full sector bank can take several seconds — use a generous timeout.
        let wipe_to = Duration::from_millis(constants::FLASH_TIMEOUT_MS * 50);
        match with_timeout(wipe_to, self.flash.wipe_storage()).await {
            Ok(Ok(_)) => {
                log::info!("Flash storage wiped successfully.");
                crate::umbilical::print_str("Flash wiped successfully.\n");
            }
            Ok(Err(e)) => {
                log::error!("Failed to wipe flash storage: {:?}", e);
                crate::umbilical::print_str("ERASE FAILED!\n");
            }
            Err(_) => {
                log::error!("Flash wipe TIMEOUT");
                crate::umbilical::print_str("ERASE TIMEOUT!\n");
            }
        }
    }

    /// Prints the current status/usage of the flash storage
    pub async fn print_flash_status(&mut self) {
        let (used, total) = self.flash.get_usage();
        let used_kb = used / 1024;
        let total_kb = total / 1024;
        let percent = (used as f32 / total as f32) * 100.0;

        let mut msg = heapless::String::<128>::new();
        let _ = core::fmt::write(&mut msg, format_args!("Flash: {}/{} KB used ({:.1}%)\n", used_kb, total_kb, percent));

        log::info!("{}", msg.as_str());
        crate::umbilical::print_str(msg.as_str());
    }
}