use arc_swap::ArcSwap;
use smol::Timer;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use smol::lock::Mutex;
use tracing::{info, error};
use smol::fs::{self, OpenOptions};
use smol::io::{AsyncWriteExt, BufWriter};

use crate::hardware::Hardware;
use crate::command::{AdcReadings, UmbilicalReadings};

// 16 ADC slots (raw,scaled per channel * 8 channels) of "N/A" plus trailing comma.
const ADC_NA_ROW: &str = "N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,";
// 23 FSW columns (connected + 20 telemetry + 2 valve states) of N/A.
const FSW_NA_ROW: &str = "N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A,N/A";

pub async fn start_logging(
    _hardware: Arc<Mutex<Hardware>>,
    adc_readings: Arc<ArcSwap<AdcReadings>>,
    umbilical_readings: Arc<ArcSwap<UmbilicalReadings>>,
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
        .unwrap_or_default()
        .as_secs();

    let mut filename = format!("{}/fill_station_log_{}.csv", log_dir, timestamp);
    let mut file_index = 1;

    // Ensure we create a new file instead of re-using one
    while std::path::Path::new(&filename).exists() {
        filename = format!("{}/fill_station_log_{}_{}.csv", log_dir, timestamp, file_index);
        file_index += 1;
    }

    let raw_file = match OpenOptions::new()
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

    // 64 KiB buffered writer — collapses 100×/s small writes into a few syscalls/s.
    let mut file = BufWriter::with_capacity(64 * 1024, raw_file);

    // Write Header
    let header = "Loop,Timestamp_ms,Igniter1_Active,Igniter2_Active,\
SV1_Open,SV1_Cont,\
ADC1_0_Raw,ADC1_0_Scaled,ADC1_1_Raw,ADC1_1_Scaled,ADC1_2_Raw,ADC1_2_Scaled,ADC1_3_Raw,ADC1_3_Scaled,\
ADC2_0_Raw,ADC2_0_Scaled,ADC2_1_Raw,ADC2_1_Scaled,ADC2_2_Raw,ADC2_2_Scaled,ADC2_3_Raw,ADC2_3_Scaled,\
FSW_Connected,FSW_Mode,FSW_Pressure,FSW_Temp,FSW_Altitude,FSW_Lat,FSW_Lon,FSW_Sats,FSW_Timestamp,\
FSW_MagX,FSW_MagY,FSW_MagZ,FSW_AccelX,FSW_AccelY,FSW_AccelZ,FSW_GyroX,FSW_GyroY,FSW_GyroZ,\
FSW_PT3,FSW_PT4,FSW_RTD,FSW_SV_Open,FSW_MAV_Open,\
QD_Enabled,QD_Direction\n";

    if let Err(e) = file.write_all(header.as_bytes()).await {
        error!("Failed to write header to log file: {}", e);
        return;
    }

    let mut loop_count: u64 = 0;

    // Reusable line buffer — pre-sized to fit a worst-case row, then reused
    // via clear() + write!. Avoids ~30 heap allocs per 10 ms tick.
    let mut line = String::with_capacity(1024);

    // Run at 100Hz
    let interval = Duration::from_millis(10);

    loop {
        let start_time = std::time::Instant::now();
        loop_count += 1;

        // 1. Snapshot ADC readings (lock-free ArcSwap load)
        let (adc_timestamp, adc_valid, adc1, adc2) = {
            let reading = adc_readings.load();
            (reading.timestamp_ms, reading.valid, reading.adc1, reading.adc2)
        };

        // 2. Snapshot hardware: clone Arcs out from under one short lock,
        //    then call GPIO-reading methods without holding the mutex.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let (ig1_active, ig2_active, sv_open, sv_cont, qd_enabled, qd_direction) = {
            let (ig1, ig2, sv1, qd) = {
                let hw = _hardware.lock().await;
                (hw.ig1.clone(), hw.ig2.clone(), hw.sv1.clone(), hw.qd_stepper.clone())
            };
            (
                ig1.is_igniting().await,
                ig2.is_igniting().await,
                sv1.is_open().await.unwrap_or(false),
                sv1.check_continuity().await.unwrap_or(false),
                qd.is_enabled().await,
                qd.get_direction().await,
            )
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let (ig1_active, ig2_active, sv_open, sv_cont, qd_enabled, qd_direction) =
            (false, false, false, false, false, false);

        // 3. Format CSV Line into reused buffer
        line.clear();
        let _ = write!(
            line,
            "{},{},{},{},{},{},",
            loop_count, adc_timestamp, ig1_active, ig2_active, sv_open, sv_cont,
        );

        // ADC channels
        if adc_valid {
            for ch in adc1.iter().chain(adc2.iter()) {
                match ch.scaled {
                    Some(v) => { let _ = write!(line, "{},{:.4},", ch.raw, v); }
                    None => { let _ = write!(line, "{},N/A,", ch.raw); }
                }
            }
        } else {
            line.push_str(ADC_NA_ROW);
        }

        // 4. FSW telemetry — lock-free ArcSwap load
        let umb_snapshot = {
            let umb = umbilical_readings.load();
            if umb.connected {
                Some(umb.telemetry.clone())
            } else {
                None
            }
        };
        match umb_snapshot {
            Some(t) => {
                let _ = write!(
                    line,
                    "true,{},{:.2},{:.2},{:.2},{:.6},{:.6},{},{:.3},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{:.2},{},{}",
                    t.flight_mode, t.pressure, t.temp, t.altitude,
                    t.latitude, t.longitude, t.num_satellites, t.gps_time,
                    t.mag_x, t.mag_y, t.mag_z,
                    t.accel_x, t.accel_y, t.accel_z,
                    t.gyro_x, t.gyro_y, t.gyro_z,
                    t.pt3, t.pt4, t.rtd,
                    t.sv_open, t.mav_open,
                );
            }
            None => line.push_str(FSW_NA_ROW),
        }

        // 5. QD stepper state
        let _ = write!(line, ",{},{}\n", qd_enabled, qd_direction);

        // Write to buffered file
        if let Err(e) = file.write_all(line.as_bytes()).await {
            error!("Failed to write to log file: {}", e);
        }

        // Flush + sync every 10 seconds (1000 samples) to prevent data loss on power cycle
        if loop_count % 1000 == 0 {
            if let Err(e) = file.flush().await {
                error!("Failed to flush log file: {}", e);
            }
            if let Err(e) = file.get_mut().sync_all().await {
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
