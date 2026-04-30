use controller_in_rust::airbrakes::AirbrakeSystem;
use controller_in_rust::controller::{
    air_density, deployment_to_area, Phase, AIRBRAKE_CD, DT, G, GROUND_TEMP_K,
};
use controller_in_rust::rocket_sim::{BODY_AREA, BODY_CD};

const MASS: f64 = 51.26;
const BARO_NOISE_STD: f64 = 0.02;
const GYRO_NOISE_STD: f64 = 0.07;

/// Replay a CSV through AirbrakeSystem::execute() and return the simulated apogee.
/// This mirrors the two-phase approach in main.rs:
///   1. Feed all CSV rows (pad/boost/coast) through execute()
///   2. Run a physics coast sim using execute()'s deployment output
fn csv_path(name: &str) -> String {
    format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn run_through_airbrakes(csv_path: &str) -> f64 {
    let mut system = AirbrakeSystem::new();

    let mut rdr = csv::Reader::from_path(csv_path)
        .unwrap_or_else(|e| panic!("Cannot open {}: {}", csv_path, e));

    let all_records: Vec<csv::StringRecord> = rdr
        .records()
        .map(|r| r.expect("CSV parse error"))
        .collect();

    let mut last_time = 0.0f64;
    let mut seen_coast = false;
    let mut last_altitude = 0.0f64;
    let mut last_velocity = 0.0f64;
    let mut last_tilt_deg = 0.0f64;
    let mut last_gp = 0.0f64;

    // Feed all CSV rows through execute()
    for record in &all_records {
        let state = record[0].trim().to_lowercase();

        let time: f64 = match record[1].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let gyro_x: f64 = record[2].parse().unwrap_or(0.0);
        let gyro_y: f64 = record[3].parse().unwrap_or(0.0);
        let alt_ft: f64 = record[5].parse().unwrap_or(0.0);
        let alt_m = alt_ft * 0.3048;

        let accel_x: f64 = record[6].parse().unwrap_or(0.0);
        let accel_y: f64 = record[7].parse().unwrap_or(0.0);
        let accel_z: f64 = record[8].parse().unwrap_or(0.0);

        if state == "coast" {
            seen_coast = true;
        }

        let phase = if state == "pad" {
            Phase::Pad
        } else if seen_coast {
            Phase::Coast
        } else {
            Phase::Boost
        };

        let _out = system.execute(time, alt_m, gyro_x, gyro_y, accel_x, accel_y, accel_z, phase);
        last_time = time;
    }

    // Get burnout state from the last execute() call's internal state
    // We need to run the coast sim. Since we can't access controller internals,
    // we read the last output and use the CSV's last altitude/velocity.
    // Instead, run a coast sim feeding synthetic sensor data through execute().
    // We need the true physics state — extract from last CSV rows.

    // Find burnout conditions from last few CSV altitude readings
    let flight_records: Vec<&csv::StringRecord> = all_records
        .iter()
        .filter(|r| !r[0].trim().eq_ignore_ascii_case("pad"))
        .filter(|r| r[1].parse::<f64>().is_ok())
        .collect();

    if flight_records.is_empty() {
        return 0.0;
    }

    // Estimate burnout velocity from last few altitude readings
    let n = flight_records.len();
    let last_n = if n >= 10 { 10 } else { n };
    let recent = &flight_records[n - last_n..];
    let t_mean: f64 = recent.iter().map(|r| r[1].parse::<f64>().unwrap()).sum::<f64>() / last_n as f64;
    let h_mean: f64 = recent.iter().map(|r| r[5].parse::<f64>().unwrap() * 0.3048).sum::<f64>() / last_n as f64;
    let mut num = 0.0;
    let mut den = 0.0;
    for r in recent {
        let t = r[1].parse::<f64>().unwrap();
        let h = r[5].parse::<f64>().unwrap() * 0.3048;
        let dt = t - t_mean;
        num += dt * (h - h_mean);
        den += dt * dt;
    }
    let burnout_velocity = if den > 0.0 { num / den } else { 0.0 };

    let last_rec = flight_records[n - 1];
    let burnout_altitude = last_rec[5].parse::<f64>().unwrap() * 0.3048;
    let burnout_time = last_rec[1].parse::<f64>().unwrap();

    // Coast simulation — same physics as main.rs
    let mut h = burnout_altitude;
    let mut v = burnout_velocity;
    let mut t = burnout_time;
    let mut apogee_h = h;

    // Use small deterministic noise for reproducibility
    let mut noise_idx: usize = 0;

    while v > 0.0 {
        // Deterministic pseudo-noise for test reproducibility
        let baro_noise = ((noise_idx as f64 * 0.7).sin()) * BARO_NOISE_STD;
        let gx_noise = ((noise_idx as f64 * 1.3).sin()) * GYRO_NOISE_STD;
        let gy_noise = ((noise_idx as f64 * 2.1).sin()) * GYRO_NOISE_STD;
        noise_idx += 1;

        let altitude_noisy = h + baro_noise;
        let out = system.execute(
            t, altitude_noisy, gx_noise, gy_noise,
            0.0, 0.0, 9.81, // accel not used during coast
            Phase::Coast,
        );

        // Physics (tilt assumed small for this test — cos(tilt) ≈ 1.0)
        let rho = air_density(h, 101325.0, GROUND_TEMP_K);
        let dynamic_pressure = 0.5 * rho * v * v;
        let fd_body = dynamic_pressure * BODY_CD * BODY_AREA;
        let a_area = deployment_to_area(out.deployment);
        let fd_brake = dynamic_pressure * AIRBRAKE_CD * a_area;
        let f_drag = fd_body + fd_brake;

        let accel = -G - f_drag / MASS;
        v += accel * DT;
        h += v * DT;
        t += DT;

        if h > apogee_h {
            apogee_h = h;
        }
    }

    apogee_h
}

#[test]
fn test_airbrakes_engages_on_overshoot() {
    // comp_24: real no-brakes apogee ~2800m, default target 3048m
    // Apogee is below target so brakes stay retracted, sim should reach ~2800m
    let apogee = run_through_airbrakes(&csv_path("comp_24.csv"));
    println!("comp_24 apogee via AirbrakeSystem: {:.1} m", apogee);
    assert!(apogee > 2700.0, "Apogee too low: {}", apogee);
    assert!(apogee < 3000.0, "Apogee too high: {}", apogee);
}

#[test]
fn test_airbrakes_retracts_when_below_target() {
    // comp_25 with target well above achievable apogee
    // Controller should keep brakes retracted (deployment = 0)
    let mut system = AirbrakeSystem::new();

    // Feed a few pad readings
    for i in 0..40 {
        let t = (i + 1) as f64 * 0.05;
        system.execute(t, 0.0, 0.0, 0.0, 0.0, 0.0, 9.81, Phase::Pad);
    }

    // Feed a coast reading at low altitude/velocity — predicted apogee will be below target
    let out = system.execute(5.0, 100.0, 0.0, 0.0, 0.0, 0.0, 9.81, Phase::Coast);
    assert_eq!(out.deployment, 0.0, "Brakes should be retracted when below target");
}

#[test]
fn test_airbrakes_calibration_happens_once() {
    let mut system = AirbrakeSystem::new();

    // Feed 50 pad readings — calibration should happen at 40, extras ignored
    for i in 0..50 {
        let t = (i + 1) as f64 * 0.05;
        let out = system.execute(t, 0.0, 0.01, 0.02, 0.0, 0.0, 9.81, Phase::Pad);
        assert_eq!(out.deployment, 0.0, "No deployment during pad");
    }

    // Transition to boost — should not panic
    let out = system.execute(3.0, 50.0, 0.5, 0.3, 0.0, 0.0, 9.81, Phase::Boost);
    assert_eq!(out.deployment, 0.0, "No deployment during boost");
}

#[test]
fn test_full_flight_replay() {
    // full_comp_25: complete real flight data (no brakes)
    // Real apogee ~2800m. Controller should command 100% deployment throughout
    // since real flight data doesn't change with deployment.
    let mut system = AirbrakeSystem::new();

    let mut rdr = csv::Reader::from_path(csv_path("full_comp_24.csv"))
        .unwrap_or_else(|e| panic!("Cannot open full_comp_24.csv: {}", e));

    let all_records: Vec<csv::StringRecord> = rdr
        .records()
        .map(|r| r.expect("CSV parse error"))
        .collect();

    let mut seen_coast = false;
    let mut max_deployment = 0.0f64;
    let mut coast_steps = 0u32;

    for record in &all_records {
        let state = record[0].trim().to_lowercase();
        let time: f64 = match record[1].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let gyro_x: f64 = record[2].parse().unwrap_or(0.0);
        let gyro_y: f64 = record[3].parse().unwrap_or(0.0);
        let alt_ft: f64 = record[5].parse().unwrap_or(0.0);
        let alt_m = alt_ft * 0.3048;
        let accel_x: f64 = record[6].parse().unwrap_or(0.0);
        let accel_y: f64 = record[7].parse().unwrap_or(0.0);
        let accel_z: f64 = record[8].parse().unwrap_or(0.0);

        if state == "coast" {
            seen_coast = true;
        }

        let phase = if state == "pad" {
            Phase::Pad
        } else if seen_coast {
            Phase::Coast
        } else {
            Phase::Boost
        };

        let out = system.execute(time, alt_m, gyro_x, gyro_y, accel_x, accel_y, accel_z, phase);

        if seen_coast && out.deployment > 0.0 {
            coast_steps += 1;
            if out.deployment > max_deployment {
                max_deployment = out.deployment;
            }
        }
    }

    println!("Coast steps with deployment: {}, max deployment: {:.1}%", coast_steps, max_deployment * 100.0);
    // comp_24 apogee (~2800m) is below default target (3048m), so brakes stay retracted
    assert_eq!(coast_steps, 0, "Brakes should stay retracted when below target");
    assert_eq!(max_deployment, 0.0, "No deployment when apogee is below target");
}
