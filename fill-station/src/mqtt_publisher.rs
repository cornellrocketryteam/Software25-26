use rumqttc::{Client, MqttOptions, QoS};
use serde::Serialize;
use smol::Timer;
use std::sync::Arc;
use std::time::Duration;
use smol::lock::Mutex;
use tracing::{info, warn, error, debug};

use crate::hardware::Hardware;
use crate::command::{AdcReadings, UmbilicalReadings};

// ============================================================================
// MQTT CONFIGURATION
// ============================================================================

/// IP address of the EMQX MQTT broker
const BROKER_HOST: &str = "192.168.0.101";

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
    timestamp: f64,
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
    sv_open: bool,
    mav_open: bool,

    // Fill station specific
    pt_1_pressure: f64,
    pt_2_pressure: f64,
    ball_valve_open: bool,
    sv_1_open: bool,
    sv_2_open: bool,
    load_cell: f64,
    ignition: bool,
    qd_state: i16,
}

// ============================================================================
// MQTT PUBLISHER TASK
// ============================================================================

pub async fn start_mqtt_publisher(
    _hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
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

    let interval = Duration::from_millis(1000 / PUBLISH_RATE_HZ);

    loop {
        let start = std::time::Instant::now();

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
                timestamp: telemetry.timestamp as f64,
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
                sv_open: telemetry.sv_open,
                mav_open: telemetry.mav_open,
                pt_1_pressure,
                pt_2_pressure,
                ball_valve_open: false,
                sv_1_open,
                sv_2_open: false,
                load_cell,
                ignition,
                qd_state: 0,
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
                timestamp: 0.0,
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
                sv_open: false,
                mav_open: false,
                pt_1_pressure,
                pt_2_pressure,
                ball_valve_open: false,
                sv_1_open,
                sv_2_open: false,
                load_cell,
                ignition,
                qd_state: 0,
            }
        };

        // 5. Serialize and publish
        match serde_json::to_string(&payload) {
            Ok(json) => {
                if let Err(e) = client.try_publish(MQTT_TOPIC, QoS::AtMostOnce, false, json.as_bytes()) {
                    debug!("MQTT publish failed (broker may be offline): {}", e);
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
