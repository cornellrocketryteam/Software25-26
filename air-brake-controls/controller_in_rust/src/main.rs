mod controller;
mod pid;
mod rocket_sim;

use controller::{
    air_density, deployment_to_area, AirbrakeController, Phase, SensorData, AIRBRAKE_AREA_MIN,
    AIRBRAKE_CD, DT, G, GROUND_TEMP_K, L, R, TARGET_APOGEE,
};
use rand::thread_rng;
use rand_distr::{Distribution, Normal};
use rocket_sim::rocket_sim;
use std::env;
use std::fs;

// Sensor noise specs (ICM-42688-P gyroscope, BMP390 barometer)
const GYRO_NOISE_STD: f64 = 0.07; // deg/s RMS
const BARO_NOISE_STD: f64 = 0.02; // Pa RMS

// Rocket mass — must match rocket_sim
const MASS: f64 = 113.0; // kg

// -----------------------------------------------------------------------------
// Inverse barometric formula: altitude (m) -> pressure (Pa)
// -----------------------------------------------------------------------------
fn altitude_to_pressure(altitude: f64, ground_pressure: f64) -> f64 {
    let t = (GROUND_TEMP_K - L * altitude).max(1.0);
    ground_pressure * (t / GROUND_TEMP_K).powf(G / (R * L))
}

// -----------------------------------------------------------------------------
// Simulation
// -----------------------------------------------------------------------------
fn run_simulation(burn_csv_path: &str, target_apogee: f64, out_csv_path: &str) -> f64 {
    let mut controller = AirbrakeController::new(target_apogee, GROUND_TEMP_K);

    // CSV output writer
    let mut wtr = csv::Writer::from_path(out_csv_path)
        .unwrap_or_else(|e| panic!("Cannot create output CSV {}: {}", out_csv_path, e));
    wtr.write_record(["time_s", "phase", "altitude_m", "velocity_ms", "deployment_pct", "drag_N", "pred_apogee_m", "error_m"])
        .unwrap();

    println!("{}", "=".repeat(88));
    println!("SIMULATED IMU FLIGHT SIMULATION");
    println!("{}", "=".repeat(88));
    println!("Burn data:     {}", burn_csv_path);
    println!("Target apogee: {:.1} m", target_apogee);
    println!("{}", "=".repeat(88));

    // Print unified table header
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
    // Phase 1: Burn replay from CSV — pass Phase::Boost to controller
    // -------------------------------------------------------------------------
    let mut rdr = csv::Reader::from_path(burn_csv_path)
        .unwrap_or_else(|e| panic!("Cannot open {}: {}", burn_csv_path, e));

    let mut last_time = 0.0f64;

    // test_25.csv columns: Timestamp, Gyro_X, Gyro_Y, Gyro_Z, "Alt, ft"
    // Derive pressure from altitude so the rest of the pipeline is unchanged.
    const SEA_LEVEL_PRESSURE_PA: f64 = 101325.0;

    for result in rdr.records() {
        let record = result.expect("CSV parse error");
        let time: f64    = record[0].parse().expect("bad time");
        let gyro_x: f64  = record[1].parse().expect("bad gyro_x");
        let gyro_y: f64  = record[2].parse().expect("bad gyro_y");
        let gyro_z: f64  = record[3].parse().expect("bad gyro_z");
        let alt_ft: f64  = record[4].parse().expect("bad alt_ft");
        let alt_m = alt_ft * 0.3048;
        // Synthesise pressure from the recorded altitude using standard ISA.
        let pressure = altitude_to_pressure(alt_m, SEA_LEVEL_PRESSURE_PA);

        let sensor_data = SensorData {
            time,
            pressure,
            gyro_x,
            gyro_y,
            gyro_z,
            phase: Phase::Boost,
        };
        controller.step(&sensor_data);
        last_time = time;

        // Print burn row from sensor buffer state
        let gp = controller.ground_pressure;
        let buf_alt = controller.sensor_buffer.last_altitude();
        let buf_vel = controller.sensor_buffer.get_velocity();
        let tilt_rad = controller.integrated_tilt.to_radians();
        let v_axial = if buf_vel > 0.0 { buf_vel / tilt_rad.cos().max(1e-6) } else { 0.0 };
        let rho = air_density(buf_alt, gp, GROUND_TEMP_K);
        let drag_burn = 0.5 * rho * v_axial * v_axial * AIRBRAKE_CD * AIRBRAKE_AREA_MIN;

        println!(
            "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:>9}  {:>8}",
            time, "BOOST", buf_alt, buf_vel, 0.0_f64, drag_burn, "---", "---"
        );
        wtr.write_record([
            format!("{:.4}", time),
            "BOOST".to_string(),
            format!("{:.3}", buf_alt),
            format!("{:.3}", buf_vel),
            format!("{:.3}", 0.0_f64),
            format!("{:.4}", drag_burn),
            "".to_string(),
            "".to_string(),
        ]).unwrap();
    }

    // Extract burnout state
    let gp = controller.ground_pressure;
    let burnout_altitude = controller.sensor_buffer.last_altitude();
    let burnout_velocity = controller.sensor_buffer.get_velocity();
    let burnout_tilt_deg = controller.integrated_tilt;

    // -------------------------------------------------------------------------
    // Phase 2: Coast simulation — pass Phase::Coast to controller
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
        // Synthetic sensor readings
        let pressure_noisy = altitude_to_pressure(h, gp) + baro_dist.sample(&mut rng);
        let gyro_x = gyro_dist.sample(&mut rng);
        let gyro_y = gyro_dist.sample(&mut rng);
        let gyro_z = gyro_dist.sample(&mut rng);

        let sensor_data = SensorData {
            time: t,
            pressure: pressure_noisy,
            gyro_x,
            gyro_y,
            gyro_z,
            phase: Phase::Coast,
        };

        // Feed to controller, get deployment
        let deployment = controller.step(&sensor_data);

        // Airspeed along rocket axis
        let v_axial = v / cos_tilt;

        // Drag along rocket axis
        let rho = air_density(h, gp, GROUND_TEMP_K);
        let a_area = deployment_to_area(deployment);
        let f_drag = 0.5 * rho * v_axial * v_axial * AIRBRAKE_CD * a_area;

        // Project drag back to vertical
        let f_drag_vertical = f_drag * cos_tilt;

        // Vertical acceleration
        let accel = -G - f_drag_vertical / MASS;

        // Integrate
        v += accel * DT;
        h += v * DT;
        t += DT;

        if h > apogee_h {
            apogee_h = h;
        }

        // Predicted apogee and error for this tick
        let pred_apogee = rocket_sim(h, v, burnout_tilt_deg, deployment, gp);
        let error = pred_apogee - target_apogee;

        println!(
            "{:7.2}  {:>6}  {:7.1}  {:7.1}  {:7.1}  {:8.2}  {:9.1}  {:+8.1}",
            t,
            "COAST",
            h,
            v,
            deployment * 100.0,
            f_drag_vertical,
            pred_apogee,
            error
        );
        wtr.write_record([
            format!("{:.4}", t),
            "COAST".to_string(),
            format!("{:.3}", h),
            format!("{:.3}", v),
            format!("{:.3}", deployment * 100.0),
            format!("{:.4}", f_drag_vertical),
            format!("{:.3}", pred_apogee),
            format!("{:.3}", error),
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

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------
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

    // Determine output path: successful_runs/run_N.csv
    let out_dir = "successful_runs";
    fs::create_dir_all(out_dir).unwrap_or_else(|e| panic!("Cannot create {}: {}", out_dir, e));

    let run_number = fs::read_dir(out_dir)
        .map(|entries| entries.filter_map(|e| e.ok()).count() + 1)
        .unwrap_or(1);

    let out_csv = format!("{}/run_{}.csv", out_dir, run_number);

    run_simulation(&burn_csv, target_apogee, &out_csv);
}
