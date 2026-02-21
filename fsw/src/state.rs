use crate::module::*;

use crate::packet::Packet;

use crate::driver::mmc56x3::Mmc56x3Sensor;
use crate::driver::bmp390::Bmp390Sensor;
use crate::driver::main_fram::Fram;
use crate::driver::lsm6dsox::Lsm6dsoxSensor;
use crate::driver::rfd900x::Rfd900x;
use crate::driver::ublox_max_m10s::UbloxMaxM10s;
use crate::driver::ads1015::Ads1015Sensor;

use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::SPI0;
use embassy_rp::spi::Spi;
use embassy_rp::uart::{Async, Uart};

use crate::actuator::{Ssa, Buzzer, Mav, SV, Chute};

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SensorState {
    OFF = 0,
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
    altimeter: Bmp390Sensor,
    pub altimeter_state: SensorState,
    pub reference_pressure: f32,
    
    // storage status
    pub sd_logging_enabled: bool,

    // fram
    fram: Fram<'static>,

    // gps
    gps: UbloxMaxM10s<'static, I2cDevice<'static>>,

    // magnetometer
    magnetometer: Mmc56x3Sensor,

    // imu
    imu: Lsm6dsoxSensor,

    // adc
    adc: Ads1015Sensor,

    // actuators
    arming_switch: Input<'static>,
    umbilical_sense: Input<'static>,
    pub arming_altitude: f32,

    pub ssa: Ssa<'static>,
    pub buzzer: Buzzer<'static>,
    pub mav: Mav<'static>,
    pub sv: SV<'static>,

    // telemetry
    radio: Rfd900x<'static>,

    // comms
    pub payload_comms_ok: bool,
    pub recovery_comms_ok: bool,
}

impl FlightState {
    pub async fn new(
        i2c_bus: &'static SharedI2c,
        spi: Spi<'static, SPI0, embassy_rp::spi::Async>,
        cs: Output<'static>,
        arming_switch: Input<'static>,
        umbilical_sense: Input<'static>,
        uart: Uart<'static, Async>,
        ssa: Ssa<'static>,
        buzzer: Buzzer<'static>,
        mav: Mav<'static>,
        sv: SV<'static>,
    ) -> Self {
        let packet = Packet::default();
        let altimeter = Bmp390Sensor::new(i2c_bus).await;
        let altimeter_init = if altimeter.is_init() {
            SensorState::VALID
        } else {
            SensorState::INVALID
        };
        let mut fram = Fram::new(spi, cs);
        let mut gps = UbloxMaxM10s::new(i2c_bus);

        // Configure GPS module to output NAV-PVT messages
        if let Err(e) = gps.configure().await {
            log::error!("Failed to configure GPS: {:?}", e);
        }

        let magnetometer = Mmc56x3Sensor::new(i2c_bus).await;
        let imu = Lsm6dsoxSensor::new(i2c_bus).await;
        let adc = Ads1015Sensor::new(i2c_bus).await;
        let radio = Rfd900x::new(uart);

        // Read stored state from FRAM
        let (stored_mode, stored_cycle_count) = match fram.read_u32(0).await {
            Ok(mode_raw) => {
                let mode = FlightMode::from_u32(mode_raw);
                log::info!("FlightMode read from FRAM: {:?}", mode);
                match fram.read_u32(4).await {
                    Ok(count) => (mode, count),
                    Err(_) => {
                        log::warn!("Failed to read CycleCount from FRAM");
                        (mode, 0)
                    }
                }
            }
            Err(_) => {
                log::warn!("Failed to read FlightMode from FRAM");
                (FlightMode::Startup, 0)
            }
        };

        Self {
            packet: packet,
            flight_mode: stored_mode,
            cycle_count: stored_cycle_count,
            key_armed: false,
            umbilical_connected: false,
            altimeter: altimeter,
            altimeter_state: altimeter_init,
            sd_logging_enabled: false, // Default to false (SD failure assumed for now)
            fram: fram,
            gps: gps,
            magnetometer: magnetometer,
            imu: imu,
            adc: adc,
            arming_switch: arming_switch,
            umbilical_sense: umbilical_sense,
            arming_altitude: 0.0,
            radio: radio,
            reference_pressure: 0.0,
            payload_comms_ok: true,
            recovery_comms_ok: true,
            ssa,
            buzzer,
            mav,
            sv,
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
        if let Err(_) = self.fram.write_u32(20, 1).await {
             log::warn!("Failed to write MAV state to FRAM");
        }
    }

    pub async fn close_mav(&mut self) {
        log::info!("ACTUATOR: Closing MAV");
        self.mav.close();
        // Closed (0)
        if let Err(_) = self.fram.write_u32(20, 0).await {
             log::warn!("Failed to write MAV state to FRAM");
        }
    }
    

    pub async fn open_sv(&mut self, duration: u64) {
        log::info!("ACTUATOR: Opening SV");
        self.sv.open(duration);
        // Open (1)
        if let Err(_) = self.fram.write_u32(24, 1).await {
             log::warn!("Failed to write SV state to FRAM");
        }
    }

    pub async fn close_sv(&mut self) {
         log::info!("ACTUATOR: Closing SV");
         self.sv.close();
         // Closed (0)
         if let Err(_) = self.fram.write_u32(24, 0).await {
             log::warn!("Failed to write SV state to FRAM");
        }
    }

    pub async fn read_sensors(&mut self) {
        self.update_actuators().await;

        // Update packet flight mode
        self.packet.flight_mode = self.flight_mode as u32;

        // Update key armed status
        self.key_armed = self.arming_switch.is_high();
        self.umbilical_connected = self.umbilical_sense.is_high();

        // Read from FRAM
        match self.fram.read_u32(0).await {
            Ok(raw) => {
                log::info!("FlightMode read from FRAM: {:?}", FlightMode::from_u32(raw));
            }
            Err(_) => {
                log::warn!("Failed to read the FlightMode from FRAM!");
            }
        }

        // Write state to FRAM
        self.write_state_to_fram().await;
        
        // Write sensor data to FRAM
        self.write_sensor_data_to_fram().await;

        // Read altimeter and update packet
        match self.altimeter.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                self.altimeter_state = SensorState::VALID;
                log::info!(
                    "BMP | Pressure = {:.2} Pa, Temp = {:.2} °C, Alt = {:.2} m",
                    self.packet.pressure,
                    self.packet.temp,
                    self.packet.altitude
                );
            }
            Err(e) => {
                self.altimeter_state = SensorState::INVALID;
                log::error!("Failed to read BMP390: {:?}", e);
            }
        }

        // Read GPS and update packet
        match self.gps.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "GPS | Lat = {:.6}°, Lon = {:.6}°, Sats = {}, Time = {:.0} s",
                    self.packet.latitude,
                    self.packet.longitude,
                    self.packet.num_satellites,
                    self.packet.timestamp
                );
            }
            Err(e) => {
                log::error!("Failed to read GPS: {:?}", e);
            }
        }

        // Read magnetometer and update packet
        match self.magnetometer.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "MAG | X = {:.2} µT, Y = {:.2} µT, Z = {:.2} µT",
                    self.packet.mag_x,
                    self.packet.mag_y,
                    self.packet.mag_z
                );
            }
            Err(e) => {
                log::error!("Failed to read AK09915 magnetometer: {:?}", e);
            }
        }

        // Read IMU and update packet
        match self.imu.read_into_packet(&mut self.packet).await {
            Ok(_) => {
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
            Err(e) => {
                log::error!("Failed to read ICM-42688-P IMU: {:?}", e);
            }
        }

        // Read ADC and update packet
        match self.adc.read_into_packet(&mut self.packet).await {
            Ok(_) => {
                log::info!(
                    "ADC | PT3={:.0} PT4={:.0} RTD={:.0} (raw)",
                    self.packet.pt3,
                    self.packet.pt4,
                    self.packet.rtd
                );
            }
            Err(e) => {
                log::error!("Failed to read ADS1015 ADC: {:?}", e);
            }
        }
    }

    pub async fn transmit(&mut self) {
        let mut data = [0u8; 80];
        data[0..4].copy_from_slice(&self.packet.flight_mode.to_le_bytes());
        data[4..8].copy_from_slice(&self.packet.pressure.to_le_bytes());
        data[8..12].copy_from_slice(&self.packet.temp.to_le_bytes());
        data[12..16].copy_from_slice(&self.packet.altitude.to_le_bytes());
        data[16..20].copy_from_slice(&self.packet.latitude.to_le_bytes());
        data[20..24].copy_from_slice(&self.packet.longitude.to_le_bytes());
        data[24..28].copy_from_slice(&self.packet.num_satellites.to_le_bytes());
        data[28..32].copy_from_slice(&self.packet.timestamp.to_le_bytes());
        data[32..36].copy_from_slice(&self.packet.mag_x.to_le_bytes());
        data[36..40].copy_from_slice(&self.packet.mag_y.to_le_bytes());
        data[40..44].copy_from_slice(&self.packet.mag_z.to_le_bytes());
        data[44..48].copy_from_slice(&self.packet.accel_x.to_le_bytes());
        data[48..52].copy_from_slice(&self.packet.accel_y.to_le_bytes());
        data[52..56].copy_from_slice(&self.packet.accel_z.to_le_bytes());
        data[56..60].copy_from_slice(&self.packet.gyro_x.to_le_bytes());
        data[60..64].copy_from_slice(&self.packet.gyro_y.to_le_bytes());
        data[64..68].copy_from_slice(&self.packet.gyro_z.to_le_bytes());
        // ADS1015 ADC channels
        data[68..72].copy_from_slice(&self.packet.pt3.to_le_bytes());
        data[72..76].copy_from_slice(&self.packet.pt4.to_le_bytes());
        data[76..80].copy_from_slice(&self.packet.rtd.to_le_bytes());

        // Transmit via radio
        match self.radio.send(&data).await {
            Ok(_) => {
                log::info!("Transmitted packet via radio");
            }
            Err(e) => {
                log::warn!("Failed to transmit packet via radio: {:?}", e);
            }
        }

        // Share telemetry with umbilical sender task (no-op if logger mode)
        crate::umbilical::update_telemetry(&data);
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

    pub async fn execute(&mut self) {
        // Read sensors and update packet
        self.read_sensors().await;
        log::info!("\n");

        // Transmit packet via radio
        self.transmit().await;
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
        if let Err(_) = self.fram.write_u32(100, alt_bits).await {
             log::warn!("Failed to write PT data to FRAM");
        }
    }

    // Reset FRAM state (FlightMode, CycleCount, Altitude log)
    pub async fn reset_fram(&mut self) {
        if let Err(_) = self.fram.reset().await {
            log::error!("Failed to reset FRAM");
        } else {
            log::info!("FRAM Reset successfully");
            self.flight_mode = FlightMode::Startup;
            self.cycle_count = 0;
        }
    }

    // Write critical state variables (Mode, CycleCount) to FRAM
    pub async fn write_state_to_fram(&mut self) {
        if let Err(_) = self.fram.write_u32(0, self.flight_mode as u32).await {
            log::warn!("Failed to write FlightMode to FRAM");
        }
        if let Err(_) = self.fram.write_u32(4, self.cycle_count).await {
            log::warn!("Failed to write CycleCount to FRAM");
        }
    }

    // Write latest sensor data (Pressure, Temp, Altitude, ADC) to FRAM
    pub async fn write_sensor_data_to_fram(&mut self) {
        let press_bits = self.packet.pressure.to_bits();
        let temp_bits = self.packet.temp.to_bits();
        let alt_bits = self.packet.altitude.to_bits();

        if let Err(_) = self.fram.write_u32(8, press_bits).await {
            //log::warn!("Failed to write Pressure to FRAM");
        }
        if let Err(_) = self.fram.write_u32(12, temp_bits).await {
            //log::warn!("Failed to write Temp to FRAM");
        }
        if let Err(_) = self.fram.write_u32(16, alt_bits).await {
            //log::warn!("Failed to write Altitude to FRAM");
        }

        // ADC channels (PT3, PT4, RTD)
        let pt3_bits = self.packet.pt3.to_bits();
        let pt4_bits = self.packet.pt4.to_bits();
        let rtd_bits = self.packet.rtd.to_bits();

        if let Err(_) = self.fram.write_u32(28, pt3_bits).await {
            //log::warn!("Failed to write PT3 to FRAM");
        }
        if let Err(_) = self.fram.write_u32(32, pt4_bits).await {
            //log::warn!("Failed to write PT4 to FRAM");
        }
        if let Err(_) = self.fram.write_u32(36, rtd_bits).await {
            //log::warn!("Failed to write RTD to FRAM");
        }
    }
}
