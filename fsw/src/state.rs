use crate::constants;
use crate::module::*;

use crate::packet::Packet;

use crate::driver::bmp390::Bmp390Sensor;
use crate::driver::main_fram::Fram;
use crate::driver::lsm6dsox::Lsm6dsoxSensor;
use crate::driver::rfd900x::Rfd900x;
use crate::driver::ublox_max_m10s::UbloxMaxM10s;
use crate::driver::ads1015::Ads1015Sensor;
use crate::driver::onboard_flash::OnboardFlash;

use embassy_rp::gpio::{Input, Output};
use embassy_rp::uart::{Async, Uart, UartTx};
use embassy_time::{Duration, with_timeout};

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
    
    // storage status
    pub sd_logging_enabled: bool,

    // fram
    fram: Fram<'static>,

    // gps
    gps: UbloxMaxM10s<'static, I2cDevice<'static>>,
    gps_ok: bool,

    // imu
    imu: Lsm6dsoxSensor,

    // adc
    adc: Ads1015Sensor,

    // actuators
    arming_switch: Input<'static>,
    umbilical_sense: Input<'static>,
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

    // External Comms
    pub payload_uart: UartTx<'static, Async>,

    #[cfg(feature = "sim_payload")]
    pub sim_radio_command: Option<crate::packet::Command>,
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi_bus: &'static SharedSpi,
        fram_cs: Output<'static>,
        altimeter_cs: Output<'static>,
        arming_switch: Input<'static>,
        umbilical_sense: Input<'static>,
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
        let fram_to = Duration::from_millis(constants::FRAM_TIMEOUT_MS);
        let flash_to = Duration::from_millis(constants::FLASH_TIMEOUT_MS);

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
        log::info!("STATE: Initializing FRAM...");
        let mut fram = Fram::new(spi_bus, fram_cs);
        log::info!("STATE: FRAM ready");
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

        // Only init I2C sensors if GPS succeeded — a GPS NACK can leave SDA stuck low,
        // hanging all subsequent I2C transactions indefinitely.
        let (imu, adc) = if gps_ok {
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
            (imu, adc)
        } else {
            log::warn!("STATE: Skipping IMU/ADC — GPS failure may have left I2C bus locked");
            (Lsm6dsoxSensor::unavailable(i2c_bus), Ads1015Sensor::unavailable(i2c_bus))
        };
        log::info!("STATE: Initializing radio (RFD900x)...");
        let radio = Rfd900x::new(uart);
        log::info!("STATE: Radio ready");

        // Read stored state from FRAM
        let (stored_mode, stored_cycle_count) = match with_timeout(fram_to, fram.read_u32(0)).await {
            Ok(Ok(mode_raw)) => {
                let mode = FlightMode::from_u32(mode_raw);
                log::info!("FlightMode read from FRAM: {:?}", mode);
                match with_timeout(fram_to, fram.read_u32(4)).await {
                    Ok(Ok(count)) => (mode, count),
                    Ok(Err(_)) => {
                        log::warn!("Failed to read CycleCount from FRAM");
                        (mode, 0)
                    }
                    Err(_) => {
                        log::warn!("FRAM read CycleCount TIMEOUT");
                        (mode, 0)
                    }
                }
            }
            Ok(Err(_)) => {
                log::warn!("Failed to read FlightMode from FRAM");
                (FlightMode::Startup, 0)
            }
            Err(_) => {
                log::warn!("FRAM read FlightMode TIMEOUT");
                (FlightMode::Startup, 0)
            }
        };

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

        Self {
            packet: packet,
            flight_mode: stored_mode,
            cycle_count: stored_cycle_count,
            key_armed: false,
            umbilical_connected: false,
            altimeter: altimeter,
            altimeter_state: altimeter_init,
            sd_logging_enabled: false,
            fram: fram,
            gps: gps,
            gps_ok,
            imu: imu,
            adc: adc,
            arming_switch: arming_switch,
            umbilical_sense: umbilical_sense,
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

    pub async fn check_subsystem_health(&mut self) {
        // TODO: Implement actual payload command checks
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
        // Open (1)
        Self::fram_write_or_warn(&mut self.fram, 20, 1, "MAV state").await;
    }

    pub async fn close_mav(&mut self) {
        log::info!("ACTUATOR: Closing MAV");
        self.mav.close();
        // Closed (0)
        Self::fram_write_or_warn(&mut self.fram, 20, 0, "MAV state").await;
    }


    pub async fn open_sv(&mut self, duration: u64) {
        log::info!("ACTUATOR: Opening SV");
        self.sv.open(duration);
        // Open (1)
        Self::fram_write_or_warn(&mut self.fram, 24, 1, "SV state").await;
    }

    pub async fn close_sv(&mut self) {
         log::info!("ACTUATOR: Closing SV");
         self.sv.close();
         // Closed (0)
         Self::fram_write_or_warn(&mut self.fram, 24, 0, "SV state").await;
    }

    /// Helper: write to FRAM with a timeout; log on error or timeout.
    async fn fram_write_or_warn(
        fram: &mut Fram<'static>,
        addr: u32,
        value: u32,
        label: &str,
    ) {
        let to = Duration::from_millis(constants::FRAM_TIMEOUT_MS);
        match with_timeout(to, fram.write_u32(addr, value)).await {
            Ok(Ok(_)) => {}
            Ok(Err(_)) => log::warn!("FRAM: write {} failed", label),
            Err(_) => log::warn!("FRAM: write {} TIMEOUT", label),
        }
    }

    pub async fn read_sensors(&mut self) {
        self.update_actuators().await;

        // Update packet flight mode
        self.packet.flight_mode = self.flight_mode as u32;

        // Update key armed status
        self.key_armed = self.arming_switch.is_high();
        self.umbilical_connected = crate::umbilical::is_connected();
        self.cfc_arm_active = self.cfc_arm.is_high();

        // Write state to FRAM
        self.write_state_to_fram().await;
        
        // Write sensor data to FRAM
        self.write_sensor_data_to_fram().await;

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
                    log::error!("GPS read TIMEOUT — I²C bus may be locked");
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

        log::info!("Flight mode: {:?}", self.flight_mode);
    }

    pub async fn transmit(&mut self) {
        let data = self.packet.to_bytes();

        match self.radio.send(&data).await {
            Ok(_) => {
                log::info!("ACK: Data transmitted successfully!");
            }
            Err(e) => {
                log::warn!("Failed to transmit packet via radio: {:?}", e);
            }
        }

        // Emit telemetry as a parseable text line over USB
        crate::umbilical::emit_telemetry(&self.packet);
    }

    pub async fn receive_radio(&mut self, buffer: &mut [u8]) -> Result<(), embassy_rp::uart::Error> {
        let result = self.radio.receive_packet(buffer).await;
        if result.is_ok() {
            log::info!("ACK: Packet received successfully!");
        }
        result
    }

    /// Receive and decode a full telemetry packet
    pub async fn receive_telemetry(&mut self) -> Result<Packet, embassy_rp::uart::Error> {
        let mut buf = [0u8; Packet::SIZE];
        self.radio.receive_packet(&mut buf).await?;
        let packet = Packet::from_bytes(&buf);
        log::info!("ACK: Telemetry packet decoded successfully!");
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
    /*
    pub async fn transition(&mut self) {
        self.flight_mode = match self.flight_mode {
            FlightMode::Startup => FlightMode::Standby,
            FlightMode::Standby => FlightMode::Ascent,
            FlightMode::Ascent => FlightMode::Coast,
            FlightMode::Coast => FlightMode::DrogueDeployed,
            FlightMode::DrogueDeployed => FlightMode::MainDeployed,
            FlightMode::MainDeployed => FlightMode::Fault,
            FlightMode::Fault => FlightMode::Startup,
        }
    }
    */

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

    // Logs critical PT (Pressure/Temperature/Altitude) data to FRAM
    pub async fn log_to_fram(&mut self) {
        let alt_bits = self.packet.altitude.to_bits();
        Self::fram_write_or_warn(&mut self.fram, 100, alt_bits, "PT data").await;
    }

    /// Read FRAM device ID and all stored fields, printing results over USB.
    /// Expected device ID: 04 7F 48 03. All-FF or all-00 means no SPI response.
    pub async fn dump_fram(&mut self) {
        let fram_to = Duration::from_millis(constants::FRAM_TIMEOUT_MS);
        // Device ID check
        match with_timeout(fram_to, self.fram.read_device_id()).await {
            Err(_) => {
                log::error!("FRAM: RDID TIMEOUT");
                crate::umbilical::print_str("FRAM: RDID TIMEOUT\n");
                return;
            }
            Ok(Ok(id)) => {
                let mut msg = heapless::String::<128>::new();
                let _ = core::fmt::write(
                    &mut msg,
                    format_args!(
                        "FRAM ID: {:02X} {:02X} {:02X} {:02X} (expect 04 7F 48 03)\n",
                        id[0], id[1], id[2], id[3]
                    ),
                );
                log::info!("{}", msg.as_str());
                crate::umbilical::print_str(msg.as_str());
            }
            Ok(Err(_)) => {
                log::error!("FRAM: no response to RDID — check HOLD# pin and wiring");
                crate::umbilical::print_str("FRAM: no response to RDID\n");
                return;
            }
        }

        // Status register (WEL bit)
        if let Ok(Ok(sr)) = with_timeout(fram_to, self.fram.read_status_register()).await {
            let mut msg = heapless::String::<64>::new();
            let _ = core::fmt::write(&mut msg, format_args!("FRAM SR: 0x{:02X} (WEL={})\n", sr, (sr >> 1) & 1));
            log::info!("{}", msg.as_str());
            crate::umbilical::print_str(msg.as_str());
        }
        embassy_time::Timer::after_millis(20).await;

        // Known stored fields
        let fields: [(&str, u32); 11] = [
            ("FlightMode", 0),
            ("CycleCount", 4),
            ("Pressure",   8),
            ("Temp",       12),
            ("Altitude",   16),
            ("MAV",        20),
            ("SV",         24),
            ("PT3",        28),
            ("PT4",        32),
            ("RTD",        36),
            ("AltLog",     100),
        ];

        for (name, addr) in fields {
            match with_timeout(fram_to, self.fram.read_u32(addr)).await {
                Ok(Ok(raw)) => {
                    let as_f32 = f32::from_bits(raw);
                    let mut msg = heapless::String::<128>::new();
                    let _ = core::fmt::write(
                        &mut msg,
                        format_args!("  [{:>3}] {:<12} raw=0x{:08X}  f32={:.3}\n", addr, name, raw, as_f32),
                    );
                    log::info!("{}", msg.as_str());
                    crate::umbilical::print_str(msg.as_str());
                }
                Ok(Err(_)) | Err(_) => {
                    let mut msg = heapless::String::<64>::new();
                    let _ = core::fmt::write(&mut msg, format_args!("  [{:>3}] {} READ FAILED/TIMEOUT\n", addr, name));
                    log::warn!("{}", msg.as_str());
                    crate::umbilical::print_str(msg.as_str());
                }
            }
            embassy_time::Timer::after_millis(10).await;
        }
        crate::umbilical::print_str("--- END FRAM DUMP ---\n");
    }

    // Reset FRAM state (FlightMode, CycleCount, Altitude log)
    pub async fn reset_fram(&mut self) {
        let to = Duration::from_millis(constants::FRAM_TIMEOUT_MS);
        match with_timeout(to, self.fram.reset()).await {
            Ok(Ok(_)) => {
                log::info!("FRAM Reset successfully");
                self.flight_mode = FlightMode::Startup;
                self.cycle_count = 0;
            }
            Ok(Err(_)) => log::error!("Failed to reset FRAM"),
            Err(_) => log::error!("FRAM reset TIMEOUT"),
        }
    }

    // Write critical state variables (Mode, CycleCount) to FRAM
    pub async fn write_state_to_fram(&mut self) {
        Self::fram_write_or_warn(&mut self.fram, 0, self.flight_mode as u32, "FlightMode").await;
        Self::fram_write_or_warn(&mut self.fram, 4, self.cycle_count, "CycleCount").await;
    }

    // Write latest sensor data (Pressure, Temp, Altitude, ADC) to FRAM
    pub async fn write_sensor_data_to_fram(&mut self) {
        let press_bits = self.packet.pressure.to_bits();
        let temp_bits = self.packet.temp.to_bits();
        let alt_bits = self.packet.altitude.to_bits();
        let pt3_bits = self.packet.pt3.to_bits();
        let pt4_bits = self.packet.pt4.to_bits();
        let rtd_bits = self.packet.rtd.to_bits();

        Self::fram_write_or_warn(&mut self.fram, 8,  press_bits, "Pressure").await;
        Self::fram_write_or_warn(&mut self.fram, 12, temp_bits,  "Temp").await;
        Self::fram_write_or_warn(&mut self.fram, 16, alt_bits,   "Altitude").await;
        Self::fram_write_or_warn(&mut self.fram, 28, pt3_bits,   "PT3").await;
        Self::fram_write_or_warn(&mut self.fram, 32, pt4_bits,   "PT4").await;
        Self::fram_write_or_warn(&mut self.fram, 36, rtd_bits,   "RTD").await;
    }

    /// Reads all stored CSV data from flash and prints it to the log
    pub async fn print_flash_dump(&mut self) {
        log::info!("--- BEGIN FLASH CSV DUMP ---");
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

            offset += chunk_size as u32;
        }
        log::info!("--- END FLASH CSV DUMP ---");
        crate::umbilical::print_str("--- END FLASH CSV DUMP ---\n");
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