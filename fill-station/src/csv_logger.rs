use smol::Timer;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use tracing::{info, error};
use smol::fs::{self, OpenOptions};
use smol::io::AsyncWriteExt;

use crate::hardware::Hardware;
use crate::command::AdcReadings;

pub async fn start_logging(
    _hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<Mutex<AdcReadings>>,
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
    let filename = format!("{}/fill_station_log_{}.csv", log_dir, timestamp);

    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
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
ADC2_0_Raw,ADC2_0_Scaled,ADC2_1_Raw,ADC2_1_Scaled,ADC2_2_Raw,ADC2_2_Scaled,ADC2_3_Raw,ADC2_3_Scaled\n";
    
    if let Err(e) = file.write_all(header.as_bytes()).await {
        error!("Failed to write header to log file: {}", e);
        return;
    }

    let mut loop_count: u64 = 0;
    
    // Run at 10Hz
    let interval = Duration::from_millis(100);

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
            for (i, ch) in adc2.iter().enumerate() {
                let scaled_str = ch.scaled.map(|v| format!("{:.4}", v)).unwrap_or("N/A".to_string());
                if i == 3 {
                    line.push_str(&format!("{},{}", ch.raw, scaled_str));
                } else {
                    line.push_str(&format!("{},{},", ch.raw, scaled_str));
                }
            }
        } else {
             // 8 channels * 2 columns = 16 N/A. 
             // We need 15 commas and 16 N/As.
             // "N/A,N/A,..."
             let nas = std::iter::repeat("N/A").take(16).collect::<Vec<_>>().join(",");
             line.push_str(&nas);
        }
        
        line.push('\n');

        // Write to file
        if let Err(e) = file.write_all(line.as_bytes()).await {
            error!("Failed to write to log file: {}", e);
        }

        // Sync to disk every 10 seconds (100 samples) to prevent data loss on power cycle
        if loop_count % 100 == 0 {
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
