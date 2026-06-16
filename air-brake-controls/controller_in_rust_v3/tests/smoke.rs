use controller_in_rust_v3::{AirbrakeSystem, Phase, SensorInput};

fn input(t: f32, alt: f32, vel_d: f32, phase: Phase) -> SensorInput {
    SensorInput {
        time: t,
        altitude: alt,
        vel_d,
        reference_pressure: 101_325.0,
        gyro_x: 0.0,
        gyro_y: 0.0,
        gyro_z: 0.0,
        accel_x: 0.0,
        accel_y: 0.0,
        accel_z: 9.80665,
        phase,
    }
}

#[test]
fn pad_returns_zero() {
    let mut sys = AirbrakeSystem::new();
    let out = sys.execute(&input(0.0, 0.0, 0.0, Phase::Pad));
    assert_eq!(out.deployment, 0.0);
}

#[test]
fn boost_returns_zero() {
    let mut sys = AirbrakeSystem::new();
    let out = sys.execute(&input(2.0, 100.0, -50.0, Phase::Boost));
    assert_eq!(out.deployment, 0.0);
}

#[test]
fn coast_descending_returns_zero() {
    let mut sys = AirbrakeSystem::new();
    // vel_d positive ⇒ descending ⇒ past apogee
    let out = sys.execute(&input(20.0, 3000.0, 50.0, Phase::Coast));
    assert_eq!(out.deployment, 0.0);
}

#[test]
fn coast_below_target_no_deploy() {
    let mut sys = AirbrakeSystem::new();
    // Slow rocket low down: ballistic apogee well under 3048 m.
    let out = sys.execute(&input(10.0, 1000.0, -50.0, Phase::Coast));
    assert_eq!(out.deployment, 0.0, "should not deploy when predicted < target");
}

#[test]
fn coast_overshooting_starts_to_deploy() {
    let mut sys = AirbrakeSystem::new();
    // High velocity so ballistic predicted apogee is well above 3048 m.
    let out = sys.execute(&input(15.0, 2000.0, -250.0, Phase::Coast));
    assert!(out.predicted_apogee > 3048.0);
    assert!(
        out.deployment > 0.0,
        "expected deployment > 0, got {}",
        out.deployment
    );
}

#[test]
fn rate_limit_caps_first_step() {
    let mut sys = AirbrakeSystem::new();
    // Single 50 ms step at MAX_RATE=2.0/s ⇒ at most 0.10 deployment increase.
    let out = sys.execute(&input(15.0, 2000.0, -300.0, Phase::Coast));
    assert!(out.deployment <= 0.11, "rate limiter too loose: {}", out.deployment);
}

#[test]
fn invalid_reference_pressure_aborts() {
    let mut sys = AirbrakeSystem::new();
    let mut bad = input(15.0, 2000.0, -250.0, Phase::Coast);
    bad.reference_pressure = 0.0;
    let out = sys.execute(&bad);
    assert_eq!(out.deployment, 0.0);
}
