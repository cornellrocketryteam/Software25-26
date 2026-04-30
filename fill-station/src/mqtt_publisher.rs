use rumqttc::{Client, MqttOptions, QoS};
use serde::Serialize;
use smol::Timer;
use std::sync::Arc;
use std::time::{Duration, Instant};
use smol::lock::Mutex;
use tracing::{info, warn, error, debug};

use crate::hardware::Hardware;
use crate::command::{ActuatorState, AdcReadings, UmbilicalReadings};

// ============================================================================
// MQTT CONFIGURATION
// ============================================================================

/// IP address of the EMQX MQTT broker
const BROKER_HOST: &str = "192.168.1.206";

/// MQTT broker port
const BROKER_PORT: u16 = 1883;

/// Publish rate in Hz
const PUBLISH_RATE_HZ: u64 = 10;

/// MQTT topic — unit_id 0 = Fill Station
const MQTT_TOPIC: &str = "rats/raw/0";

/// MQTT client identifier
const CLIENT_ID: &str = "fill-station";

// ============================================================================
// JSON PAYLOAD — matches the TimescaleDB schema exactly
// ============================================================================

/// JSON payload published to the EMQX broker.
/// Field names must match the EMQX rule SQL (`payload.field_name`).
#[derive(Serialize)]
struct TelemetryPayload {
    // Top-level radio (not available on fill station — no RFD900x)
    sync_word: u32,

    // Shared telemetry (from FSW via umbilical)
    flight_mode: u32,
    pressure: f64,
    temp: f64,
    altitude: f64,
    latitude: f64,
    longitude: f64,
    num_satellites: u32,
    #[serde(rename = "timestamp")]
    gps_time: f64,
    mag_x: f64,
    mag_y: f64,
    mag_z: f64,
    accel_x: f64,
    accel_y: f64,
    accel_z: f64,
    gyro_x: f64,
    gyro_y: f64,
    gyro_z: f64,
    pt3: f64,
    pt4: f64,
    rtd: f64,
    sv_2_open: bool,
    mav_open: bool,
    ms_since_boot_cfc: u32,

    // Event flags
    ssa_drogue_deployed: u8,
    ssa_main_deployed: u8,
    cmd_n1: u8,
    cmd_n2: u8,
    cmd_n3: u8,
    cmd_n4: u8,
    cmd_a1: u8,
    cmd_a2: u8,
    cmd_a3: u8,

    // Airbrake & control
    airbrake_deployment: f64,
    predicted_apogee: f64,

    // Advanced GPS / u-blox
    h_acc: u32,
    v_acc: u32,
    vel_n: f64,
    vel_e: f64,
    vel_d: f64,
    g_speed: f64,
    s_acc: u32,
    head_acc: u32,
    fix_type: u8,
    head_mot: i32,

    // BLiMS outputs
    blims_motor_position: f64,
    blims_phase_id: i8,
    blims_pid_p: f64,
    blims_pid_i: f64,
    blims_bearing: f64,
    blims_loiter_step: i8,
    blims_heading_des: f64,
    blims_heading_error: f64,
    blims_error_integral: f64,
    blims_dist_to_target_m: f64,

    // BLiMS config
    blims_target_lat: f64,
    blims_target_lon: f64,
    blims_wind_from_deg: f64,

    // Fill station specific
    pt_1_pressure: f64,
    pt_2_pressure: f64,
    ball_valve_open: bool,
    sv_1_open: bool,
    load_cell: f64,
    ignition: bool,
    qd_state: i16,
    ms_since_boot_fill: u64,
}

// ============================================================================
// MQTT PUBLISHER TASK
// ============================================================================

pub async fn start_mqtt_publisher(
    _hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
    actuator_state: Arc<ActuatorState>,
) {
    info!("Starting MQTT publisher task ({}:{}, topic: {}, rate: {} Hz)",
          BROKER_HOST, BROKER_PORT, MQTT_TOPIC, PUBLISH_RATE_HZ);

    let mut mqttoptions = MqttOptions::new(CLIENT_ID, BROKER_HOST, BROKER_PORT);
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    // Spawn a dedicated OS thread for the MQTT event loop.
    // This handles CONNECT, CONNACK, PINGREQ/PINGRESP, and auto-reconnection.
    std::thread::spawn(move || {
        for notification in connection.iter() {
            match notification {
                Ok(event) => {
                    debug!("MQTT event: {:?}", event);
                }
                Err(e) => {
                    error!("MQTT connection error: {} — will auto-reconnect", e);
                    // Sleep briefly to avoid busy-looping on persistent errors
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
        warn!("MQTT event loop exited");
    });

    info!("MQTT event loop started on background thread");

    let boot_time = Instant::now();
    let interval = Duration::from_millis(1000 / PUBLISH_RATE_HZ);

    loop {
        let start = Instant::now();
        let ms_since_boot_fill = boot_time.elapsed().as_millis() as u64;

        // 1. Gather ADC data
        let (pt_1_pressure, pt_2_pressure, load_cell) = {
            let adc = adc_readings.lock().await;
            if adc.valid {
                (
                    adc.adc1[0].scaled.unwrap_or(0.0) as f64, // PT1500
                    adc.adc1[1].scaled.unwrap_or(0.0) as f64, // PT1000
                    adc.adc2[1].scaled.unwrap_or(0.0) as f64, // Load cell
                )
            } else {
                (0.0, 0.0, 0.0)
            }
        };

        // 2. Gather FSW telemetry from umbilical
        let (fsw_connected, telemetry) = {
            let umb = umbilical_readings.lock().await;
            (umb.connected, umb.telemetry)
        };

        // 3a. Gather last-commanded actuator state (ball valve, QD)
        let ball_valve_open = actuator_state.ball_valve_open.load(std::sync::atomic::Ordering::Relaxed);
        let qd_state = actuator_state.qd_state.load(std::sync::atomic::Ordering::Relaxed);

        // 3. Gather hardware state (SV1, igniters)
        let (sv_1_open, ignition) = {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = _hardware.lock().await;
                let sv1 = hw.sv1.is_open().await.unwrap_or(false);
                let ig = hw.ig1.is_igniting().await;
                (sv1, ig)
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                (false, false)
            }
        };

        // 4. Build payload
        let payload = if fsw_connected {
            TelemetryPayload {
                sync_word: 0,
                flight_mode: telemetry.flight_mode,
                pressure: telemetry.pressure as f64,
                temp: telemetry.temp as f64,
                altitude: telemetry.altitude as f64,
                latitude: telemetry.latitude as f64,
                longitude: telemetry.longitude as f64,
                num_satellites: telemetry.num_satellites,
                gps_time: telemetry.gps_time as f64,
                mag_x: telemetry.mag_x as f64,
                mag_y: telemetry.mag_y as f64,
                mag_z: telemetry.mag_z as f64,
                accel_x: telemetry.accel_x as f64,
                accel_y: telemetry.accel_y as f64,
                accel_z: telemetry.accel_z as f64,
                gyro_x: telemetry.gyro_x as f64,
                gyro_y: telemetry.gyro_y as f64,
                gyro_z: telemetry.gyro_z as f64,
                pt3: telemetry.pt3 as f64,
                pt4: telemetry.pt4 as f64,
                rtd: telemetry.rtd as f64,
                sv_2_open: telemetry.sv_open,
                mav_open: telemetry.mav_open,
                ms_since_boot_cfc: telemetry.ms_since_boot_cfc,
                ssa_drogue_deployed: telemetry.ssa_drogue_deployed,
                ssa_main_deployed: telemetry.ssa_main_deployed,
                cmd_n1: telemetry.cmd_n1,
                cmd_n2: telemetry.cmd_n2,
                cmd_n3: telemetry.cmd_n3,
                cmd_n4: telemetry.cmd_n4,
                cmd_a1: telemetry.cmd_a1,
                cmd_a2: telemetry.cmd_a2,
                cmd_a3: telemetry.cmd_a3,
                airbrake_deployment: telemetry.airbrake_deployment as f64,
                predicted_apogee: telemetry.predicted_apogee as f64,
                h_acc: telemetry.h_acc,
                v_acc: telemetry.v_acc,
                vel_n: telemetry.vel_n,
                vel_e: telemetry.vel_e,
                vel_d: telemetry.vel_d,
                g_speed: telemetry.g_speed,
                s_acc: telemetry.s_acc,
                head_acc: telemetry.head_acc,
                fix_type: telemetry.fix_type,
                head_mot: telemetry.head_mot,
                blims_motor_position: telemetry.blims_motor_position as f64,
                blims_phase_id: telemetry.blims_phase_id,
                blims_pid_p: telemetry.blims_pid_p as f64,
                blims_pid_i: telemetry.blims_pid_i as f64,
                blims_bearing: telemetry.blims_bearing as f64,
                blims_loiter_step: telemetry.blims_loiter_step,
                blims_heading_des: telemetry.blims_heading_des as f64,
                blims_heading_error: telemetry.blims_heading_error as f64,
                blims_error_integral: telemetry.blims_error_integral as f64,
                blims_dist_to_target_m: telemetry.blims_dist_to_target_m as f64,
                blims_target_lat: telemetry.blims_target_lat as f64,
                blims_target_lon: telemetry.blims_target_lon as f64,
                blims_wind_from_deg: telemetry.blims_wind_from_deg as f64,
                pt_1_pressure,
                pt_2_pressure,
                ball_valve_open,
                sv_1_open,
                load_cell,
                ignition,
                qd_state,
                ms_since_boot_fill,
            }
        } else {
            // FSW not connected — still publish fill station data with zeroed FSW fields
            TelemetryPayload {
                sync_word: 0,
                flight_mode: 0,
                pressure: 0.0,
                temp: 0.0,
                altitude: 0.0,
                latitude: 0.0,
                longitude: 0.0,
                num_satellites: 0,
                gps_time: 0.0,
                mag_x: 0.0,
                mag_y: 0.0,
                mag_z: 0.0,
                accel_x: 0.0,
                accel_y: 0.0,
                accel_z: 0.0,
                gyro_x: 0.0,
                gyro_y: 0.0,
                gyro_z: 0.0,
                pt3: 0.0,
                pt4: 0.0,
                rtd: 0.0,
                sv_2_open: false,
                mav_open: false,
                ms_since_boot_cfc: 0,
                ssa_drogue_deployed: 0,
                ssa_main_deployed: 0,
                cmd_n1: 0,
                cmd_n2: 0,
                cmd_n3: 0,
                cmd_n4: 0,
                cmd_a1: 0,
                cmd_a2: 0,
                cmd_a3: 0,
                airbrake_deployment: 0.0,
                predicted_apogee: 0.0,
                h_acc: 0,
                v_acc: 0,
                vel_n: 0.0,
                vel_e: 0.0,
                vel_d: 0.0,
                g_speed: 0.0,
                s_acc: 0,
                head_acc: 0,
                fix_type: 0,
                head_mot: 0,
                blims_motor_position: 0.0,
                blims_phase_id: 0,
                blims_pid_p: 0.0,
                blims_pid_i: 0.0,
                blims_bearing: 0.0,
                blims_loiter_step: 0,
                blims_heading_des: 0.0,
                blims_heading_error: 0.0,
                blims_error_integral: 0.0,
                blims_dist_to_target_m: 0.0,
                blims_target_lat: 0.0,
                blims_target_lon: 0.0,
                blims_wind_from_deg: 0.0,
                pt_1_pressure,
                pt_2_pressure,
                ball_valve_open,
                sv_1_open,
                load_cell,
                ignition,
                qd_state,
                ms_since_boot_fill,
            }
        };

        // 5. Serialize and publish
        match serde_json::to_string(&payload) {
            Ok(json) => {
                if let Err(e) = client.try_publish(MQTT_TOPIC, QoS::AtMostOnce, false, json.as_bytes()) {
                    warn!("MQTT publish failed (broker may be offline): {}", e);
                }
            }
            Err(e) => {
                error!("Failed to serialize MQTT payload: {}", e);
            }
        }

        // 6. Sleep for remainder of interval
        let elapsed = start.elapsed();
        if elapsed < interval {
            Timer::after(interval - elapsed).await;
        }
    }
}
