use smol::Timer;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use tracing::{info, error};
use smol::fs::{self, OpenOptions};
use smol::io::AsyncWriteExt;

use crate::hardware::Hardware;
use crate::command::{AdcReadings, UmbilicalReadings};

pub async fn start_logging(
    _hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
    umbilical_readings: Arc<Mutex<UmbilicalReadings>>,
) {
    info!("Starting CSV Logger task...");

    // Create logs directory if it doesn't exist
    #[cfg(target_os = "linux")]
    let log_dir = "/tmp/data";
    #[cfg(not(target_os = "linux"))]
    let log_dir = "logs";

    fs::create_dir_all(log_dir).await.ok();

    // Create filename with timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
        
    let mut filename = format!("{}/fill_station_log_{}.csv", log_dir, timestamp);
    let mut file_index = 1;

    // Ensure we create a new file instead of re-using one
    while std::path::Path::new(&filename).exists() {
        filename = format!("{}/fill_station_log_{}_{}.csv", log_dir, timestamp, file_index);
        file_index += 1;
    }

    let mut file = match OpenOptions::new()
        .create(true) // this creates the file if it doesn't exist
        .write(true)
        .truncate(true) // Ensure we start with a clean file
        .open(&filename)
        .await
    {
        Ok(f) => {
            info!("Created log file: {}", filename);
            f
        },
        Err(e) => {
            error!("Failed to create log file: {}", e);
            return;
        }
    };

    // Write Header
    // SV columns: SV1_Actuated, SV1_Cont, ...
    let header = "Loop,Timestamp_ms,MAV_Angle,MAV_Pulse_US,Igniter1_Active,Igniter2_Active,\
SV1_Actuated,SV1_Cont,SV2_Actuated,SV2_Cont,SV3_Actuated,SV3_Cont,SV4_Actuated,SV4_Cont,SV5_Actuated,SV5_Cont,\
ADC1_0_Raw,ADC1_0_Scaled,ADC1_1_Raw,ADC1_1_Scaled,ADC1_2_Raw,ADC1_2_Scaled,ADC1_3_Raw,ADC1_3_Scaled,\
ADC2_0_Raw,ADC2_0_Scaled,ADC2_1_Raw,ADC2_1_Scaled,ADC2_2_Raw,ADC2_2_Scaled,ADC2_3_Raw,ADC2_3_Scaled,\
FSW_Connected,FSW_Mode,FSW_Pressure,FSW_Temp,FSW_Altitude,FSW_Lat,FSW_Lon,FSW_Sats,FSW_Timestamp,\
FSW_MagX,FSW_MagY,FSW_MagZ,FSW_AccelX,FSW_AccelY,FSW_AccelZ,FSW_GyroX,FSW_GyroY,FSW_GyroZ,\
FSW_PT3,FSW_PT4,FSW_RTD\n";
    
    if let Err(e) = file.write_all(header.as_bytes()).await {
        error!("Failed to write header to log file: {}", e);
        return;
    }

    let mut loop_count: u64 = 0;
    
    // Run at 100Hz
    let interval = Duration::from_millis(10);

    loop {
        let start_time = std::time::Instant::now();
        loop_count += 1;

        // 1. Gather ADC Data
        let (adc_timestamp, adc_valid, adc1, adc2) = {
            let reading = adc_readings.lock().await;
            (reading.timestamp_ms, reading.valid, reading.adc1, reading.adc2)
        };

        // 2. Gather Hardware Data (MAV, SV, Igniters)
        // We lock hardware briefly
        let (mav_angle, mav_pulse, ig1_active, ig2_active, sv_states) = {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let hw = _hardware.lock().await;
                
                // MAV
                let mav_pulse = hw.mav.get_pulse_width_us().await.unwrap_or(0);
                let mav_angle = hw.mav.get_angle().await.unwrap_or(0.0);

                // Igniters
                let ig1 = hw.ig1.is_igniting().await;
                let ig2 = hw.ig2.is_igniting().await;

                // SVs (Actuated, Continuity)
                let sv1 = (hw.sv1.is_actuated().await.unwrap_or(false), hw.sv1.check_continuity().await.unwrap_or(false));
                let sv2 = (hw.sv2.is_actuated().await.unwrap_or(false), hw.sv2.check_continuity().await.unwrap_or(false));
                let sv3 = (hw.sv3.is_actuated().await.unwrap_or(false), hw.sv3.check_continuity().await.unwrap_or(false));
                let sv4 = (hw.sv4.is_actuated().await.unwrap_or(false), hw.sv4.check_continuity().await.unwrap_or(false));
                // SV5 Logic inverted as per main.rs
                let sv5_act = hw.sv5.is_actuated().await.unwrap_or(false);
                let sv5 = (!sv5_act, hw.sv5.check_continuity().await.unwrap_or(false));

                (mav_angle, mav_pulse, ig1, ig2, [sv1, sv2, sv3, sv4, sv5])
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                (0.0, 0, false, false, [(false,false); 5])
            }
        };

        // Format CSV Line
        let mut line = format!("{},{},{:.2},{},{},{},", 
            loop_count, adc_timestamp, mav_angle, mav_pulse, ig1_active, ig2_active);

        // Append SVs
        for (act, cont) in sv_states {
            line.push_str(&format!("{},{},", act, cont));
        }

        // Append ADCs
        if adc_valid {
            for ch in adc1 {
                let scaled_str = ch.scaled.map(|v| format!("{:.4}", v)).unwrap_or("N/A".to_string());
                line.push_str(&format!("{},{},", ch.raw, scaled_str));
            }
            for ch in adc2 {
                let scaled_str = ch.scaled.map(|v| format!("{:.4}", v)).unwrap_or("N/A".to_string());
                line.push_str(&format!("{},{},", ch.raw, scaled_str));
            }
        } else {
             // 8 channels * 2 columns = 16 N/A + trailing comma
             let nas = std::iter::repeat("N/A").take(16).collect::<Vec<_>>().join(",");
             line.push_str(&nas);
             line.push(',');
        }

        // 3. Gather FSW telemetry
        {
            let umb = umbilical_readings.lock().await;
            if umb.connected {
                let t = &umb.telemetry;
                line.push_str(&format!(
                    "true,{},{:.2},{:.2},{:.2},{:.6},{:.6},{},{:.3},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{:.2}",
                    t.flight_mode, t.pressure, t.temp, t.altitude,
                    t.latitude, t.longitude, t.num_satellites, t.timestamp,
                    t.mag_x, t.mag_y, t.mag_z,
                    t.accel_x, t.accel_y, t.accel_z,
                    t.gyro_x, t.gyro_y, t.gyro_z,
                    t.pt3, t.pt4, t.rtd,
                ));
            } else {
                // 21 FSW columns: connected + 20 telemetry fields
                let nas = std::iter::repeat("N/A").take(21).collect::<Vec<_>>().join(",");
                line.push_str(&nas);
            }
        }

        line.push('\n');

        // Write to file
        if let Err(e) = file.write_all(line.as_bytes()).await {
            error!("Failed to write to log file: {}", e);
        }

        // Sync to disk every 10 seconds (1000 samples) to prevent data loss on power cycle
        if loop_count % 1000 == 0 {
            if let Err(e) = file.sync_all().await {
                 error!("Failed to sync log file: {}", e);
            }
        }

        // Sleep
        let elapsed = start_time.elapsed();
        if elapsed < interval {
            Timer::after(interval - elapsed).await;
        }
    }
}
