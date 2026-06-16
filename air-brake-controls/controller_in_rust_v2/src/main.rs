pub mod airbrakes;
mod controller;
mod gyro_calibration;
mod measure_tilt;
mod rocket_sim;

use controller::{
    air_density, deployment_to_area, AirbrakeController, Phase, SensorData,
    AIRBRAKE_AREA_MIN, AIRBRAKE_CD, DT, G, GROUND_TEMP_K, TARGET_APOGEE,
};
use gyro_calibration::{compute_drift, PAD_CALIBRATION_COUNT};
use measure_tilt::measure_tilt;
use rand::thread_rng;
use rand_distr::{Distribution, Normal};
use rocket_sim::{BODY_CD, BODY_AREA};
use std::env;
use std::fs;

const GYRO_NOISE_STD: f64 = 0.07;  // deg/s RMS
const BARO_NOISE_STD: f64 = 0.02;  // m RMS
const MASS: f64 = 51.26;           // kg (113 lb)

fn run_simulation(burn_csv_path: &str, target_apogee: f64, out_csv_path: &str) -> f64 {
    let mut controller = AirbrakeController::new(target_apogee, GROUND_TEMP_K);

    let mut wtr = csv::Writer::from_path(out_csv_path)
        .unwrap_or_else(|e| panic!("Cannot create output CSV {}: {}", out_csv_path, e));
    wtr.write_record(["time_s", "phase", "altitude_m", "velocity_ms", "deployment_pct", "drag_N", "pred_apogee_m", "error_m"])
        .unwrap();

    println!("{}", "=".repeat(88));
    println!("AIRBRAKE FLIGHT SIMULATION");
    println!("{}", "=".repeat(88));
    println!("Burn data:     {}", burn_csv_path);
    println!("Target apogee: {:.1} m", target_apogee);
    println!("{}", "=".repeat(88));

    println!();
    println!("{}", "-".repeat(88));
    println!(
        "{:>7}  {:>6}  {:>7}  {:>7}  {:>7}  {:>8}  {:>9}  {:>8}",
        "Time", "Phase", "Alt", "Vel", "Deploy", "Drag", "PredApog", "Error"
    );
    println!(
        "{:>7}  {:>6}  {:>7}  {:>7}  {:>7}  {:>8}  {:>9}  {:>8}",
        "(s)", "", "(m)", "(m/s)", "(%)", "(N)", "(m)", "(m)"
    );
    println!("{}", "-".repeat(88));

    // -------------------------------------------------------------------------
    // Load all CSV records
    // -------------------------------------------------------------------------
    let mut rdr = csv::Reader::from_path(burn_csv_path)
        .unwrap_or_else(|e| panic!("Cannot open {}: {}", burn_csv_path, e));

    let all_records: Vec<csv::StringRecord> = rdr
        .records()
        .map(|r| r.expect("CSV parse error"))
        .collect();

    let mut last_time = 0.0f64;
    let mut seen_coast = false;

    // -------------------------------------------------------------------------
    // Phase 1: Pad — collect first 40 pad rows, feed to controller, calibrate
    // -------------------------------------------------------------------------
    let mut pad_readings: Vec<(f64, f64, f64)> = Vec::new();
    let mut accel_readings: Vec<(f64, f64, f64)> = Vec::new();

    for record in all_records.iter().filter(|r| r[0].trim().eq_ignore_ascii_case("pad")).take(PAD_CALIBRATION_COUNT) {
        let time: f64   = record[1].parse().expect("bad time");
        let gyro_x: f64 = record[2].parse().expect("bad gyro_x");
        let gyro_y: f64 = record[3].parse().expect("bad gyro_y");
        let alt_ft: f64 = record[5].parse().expect("bad alt_ft");
        let alt_m = alt_ft * 0.3048;
        let accel_x: f64 = record[6].parse().expect("bad accel_x");
        let accel_y: f64 = record[7].parse().expect("bad accel_y");
        let accel_z: f64 = record[8].parse().expect("bad accel_z");

        pad_readings.push((time, gyro_x, gyro_y));
        accel_readings.push((accel_x, accel_y, accel_z));

        let sensor_data = SensorData { time, altitude: alt_m, gyro_x, gyro_y, phase: Phase::Pad };
        let _out = controller.step(&sensor_data);
        last_time = time;

        let gp = controller.ground_pressure;
        let buf_alt = controller.sensor_buffer.last_altitude();
        let buf_vel = controller.sensor_buffer.get_velocity();
        let tilt_rad = controller.integrated_tilt.to_radians();
        let v_axial = if buf_vel > 0.0 { buf_vel / tilt_rad.cos().max(1e-6) } else { 0.0 };
        let rho = air_density(buf_alt, gp, GROUND_TEMP_K);
        let drag_burn = 0.5 * rho * v_axial * v_axial * AIRBRAKE_CD * AIRBRAKE_AREA_MIN;

        println!(
            "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:>9}  {:>8}",
            time, "PAD", buf_alt, buf_vel, 0.0_f64, drag_burn, "---", "---"
        );
        wtr.write_record([
            format!("{:.4}", time), "PAD".to_string(),
            format!("{:.3}", buf_alt), format!("{:.3}", buf_vel),
            format!("{:.3}", 0.0_f64), format!("{:.4}", drag_burn),
            "".to_string(), "".to_string(),
        ]).unwrap();
    }

    let drift = compute_drift(&pad_readings);
    let drift_x = drift.x;
    let drift_y = drift.y;

    let tilt = measure_tilt(&accel_readings);
    controller.set_beginning_tilt(tilt.x_deg, tilt.y_deg);

    println!(
        "[GYRO CAL]  drift_x={:+.6} drift_y={:+.6} (deg/s/s)",
        drift_x, drift_y
    );
    println!(
        "[TILT CAL]  tilt_x={:.4}° tilt_y={:.4}° total={:.4}°",
        tilt.x_deg, tilt.y_deg, controller.beginning_tilt
    );

    // -------------------------------------------------------------------------
    // Phase 2: Flight — boost and coast rows, drift-corrected gyro
    // -------------------------------------------------------------------------
    for record in all_records.iter().filter(|r| !r[0].trim().eq_ignore_ascii_case("pad")) {
        let state    = record[0].trim().to_lowercase();
        let time: f64 = match record[1].parse() {
            Ok(v) => v,
            Err(_) => continue, // skip malformed rows
        };
        let gyro_x: f64  = record[2].parse::<f64>().expect("bad gyro_x") - drift_x;
        let gyro_y: f64  = record[3].parse::<f64>().expect("bad gyro_y") - drift_y;
        let alt_ft: f64  = record[5].parse().expect("bad alt_ft");
        let alt_m = alt_ft * 0.3048;

        if state == "coast" {
            seen_coast = true;
        }
        let phase = if seen_coast { Phase::Coast } else { Phase::Boost };

        let sensor_data = SensorData { time, altitude: alt_m, gyro_x, gyro_y, phase };
        let out = controller.step(&sensor_data);
        last_time = time;

        let gp = controller.ground_pressure;
        let buf_alt = controller.sensor_buffer.last_altitude();
        let buf_vel = controller.sensor_buffer.get_velocity();
        let tilt_rad = controller.integrated_tilt.to_radians();
        let v_axial = if buf_vel > 0.0 { buf_vel / tilt_rad.cos().max(1e-6) } else { 0.0 };
        let rho = air_density(buf_alt, gp, GROUND_TEMP_K);
        let a_area = deployment_to_area(out.deployment);
        let drag = 0.5 * rho * v_axial * v_axial * AIRBRAKE_CD * a_area;

        let phase_label = if state == "coast" { "COAST" } else { "BOOST" };

        if seen_coast && buf_vel > 0.0 {
            println!(
                "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:9.1}  {:+8.1}",
                time, phase_label, buf_alt, buf_vel, out.deployment * 100.0, drag, out.predicted_apogee, out.error
            );
            wtr.write_record([
                format!("{:.4}", time), phase_label.to_string(),
                format!("{:.3}", buf_alt), format!("{:.3}", buf_vel),
                format!("{:.3}", out.deployment * 100.0), format!("{:.4}", drag),
                format!("{:.3}", out.predicted_apogee), format!("{:.3}", out.error),
            ]).unwrap();
        } else {
            println!(
                "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:>9}  {:>8}",
                time, phase_label, buf_alt, buf_vel, out.deployment * 100.0, drag, "---", "---"
            );
            wtr.write_record([
                format!("{:.4}", time), phase_label.to_string(),
                format!("{:.3}", buf_alt), format!("{:.3}", buf_vel),
                format!("{:.3}", out.deployment * 100.0), format!("{:.4}", drag),
                "".to_string(), "".to_string(),
            ]).unwrap();
        }
    }

    let gp = controller.ground_pressure;
    let burnout_altitude = controller.sensor_buffer.last_altitude();
    let burnout_velocity = controller.sensor_buffer.get_velocity();
    let burnout_tilt_deg = controller.integrated_tilt;

    // -------------------------------------------------------------------------
    // Phase 2: Coast simulation
    // -------------------------------------------------------------------------
    let mut h = burnout_altitude;
    let mut v = burnout_velocity;
    let tilt_rad = burnout_tilt_deg.to_radians();
    let cos_tilt = tilt_rad.cos();
    let mut t = last_time;
    let mut apogee_h = h;

    let baro_dist = Normal::new(0.0, BARO_NOISE_STD).unwrap();
    let gyro_dist = Normal::new(0.0, GYRO_NOISE_STD).unwrap();
    let mut rng = thread_rng();

    while v > 0.0 {
        let altitude_noisy = h + baro_dist.sample(&mut rng);
        let gyro_x = gyro_dist.sample(&mut rng);
        let gyro_y = gyro_dist.sample(&mut rng);
        let sensor_data = SensorData {
            time: t,
            altitude: altitude_noisy,
            gyro_x,
            gyro_y,
            phase: Phase::Coast,
        };

        let out = controller.step(&sensor_data);

        let v_axial = v / cos_tilt;
        let rho = air_density(h, gp, GROUND_TEMP_K);
        let dynamic_pressure = 0.5 * rho * v_axial * v_axial;

        // Body drag (must match rocket_sim.rs)
        let fd_body = dynamic_pressure * BODY_CD * BODY_AREA;

        // Airbrake drag
        let a_area = deployment_to_area(out.deployment);
        let fd_brake = dynamic_pressure * AIRBRAKE_CD * a_area;

        let f_drag_vertical = (fd_body + fd_brake) * cos_tilt;

        let accel = -G - f_drag_vertical / MASS;

        v += accel * DT;
        h += v * DT;
        t += DT;

        if h > apogee_h {
            apogee_h = h;
        }

        println!(
            "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:9.1}  {:+8.1}",
            t, "COAST", h, v, out.deployment * 100.0, f_drag_vertical, out.predicted_apogee, out.error
        );
        wtr.write_record([
            format!("{:.4}", t),
            "COAST".to_string(),
            format!("{:.3}", h),
            format!("{:.3}", v),
            format!("{:.3}", out.deployment * 100.0),
            format!("{:.4}", f_drag_vertical),
            format!("{:.3}", out.predicted_apogee),
            format!("{:.3}", out.error),
        ]).unwrap();
    }

    println!("{}", "-".repeat(88));
    println!();
    println!(
        "[APOGEE]  t={:.2}s,  altitude={:.1} m",
        t, apogee_h
    );
    println!("  Target:  {:.1} m", target_apogee);
    println!(
        "  Error:   {:+.1} m  ({:+.2}%)",
        apogee_h - target_apogee,
        (apogee_h / target_apogee - 1.0) * 100.0
    );
    println!("{}", "=".repeat(88));

    wtr.flush().unwrap();
    println!("Run saved to: {}", out_csv_path);

    apogee_h
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let burn_csv = if args.len() > 1 {
        args[1].clone()
    } else {
        "../comp_25_clean.csv".to_string()
    };

    let target_apogee = if args.len() > 2 {
        args[2].parse::<f64>().unwrap_or(TARGET_APOGEE)
    } else {
        TARGET_APOGEE
    };

    let out_dir = "successful_runs";
    fs::create_dir_all(out_dir).unwrap_or_else(|e| panic!("Cannot create {}: {}", out_dir, e));

    let run_number = fs::read_dir(out_dir)
        .map(|entries| entries.filter_map(|e| e.ok()).count() + 1)
        .unwrap_or(1);

    let out_csv = format!("{}/run_{}.csv", out_dir, run_number);

    run_simulation(&burn_csv, target_apogee, &out_csv);
}
