// Rocket body aerodynamics (6-inch diameter)
const BODY_CD: f64 = 0.4;
const BODY_DIAMETER: f64 = 0.1524;                                    // 6 inches in metres
const BODY_AREA: f64 = std::f64::consts::PI * (BODY_DIAMETER / 2.0) * (BODY_DIAMETER / 2.0); // ~0.01824 m²

/// Simulates coast phase from given conditions and returns predicted apogee (m).
pub fn rocket_sim(x: f64, v: f64, tilt_deg: f64, airbrake: f64, ground_pressure: f64) -> f64 {
    const DT: f64 = 0.01;
    const G: f64 = 9.80665;
    const R: f64 = 287.05;
    const L: f64 = 0.0065;
    const MASS: f64 = 113.0;
    const T0: f64 = 288.15;
    let p0_pa = ground_pressure;

    // ISA air density — same formula as controller.rs
    let air_density = |h: f64| -> f64 {
        let t = (T0 - L * h).max(1.0);
        let p = p0_pa * (t / T0).powf(G / (R * L));
        (p / (R * t)).max(0.001)
    };

    // Airbrake contribution only (body drag is added separately)
    let airbrake_coeff_area = |deploy: f64| -> (f64, f64) {
        (0.4, 0.001848 + (0.021935 - 0.001848) * deploy) // 2.86479 in² to 34 in²
    };

    let cos_tilt = tilt_deg.abs().to_radians().cos().max(1e-6);

    let mut x = x;
    let mut v = v;

    loop {
        // Air density at current altitude
        let rho = air_density(x);

        // Airspeed along rocket axis (vertical velocity is one leg, axial is hypotenuse)
        let v_air = v.abs() / cos_tilt;
        let dynamic_pressure = 0.5 * rho * v_air * v_air;

        // Body drag
        let fd_body = dynamic_pressure * BODY_CD * BODY_AREA;

        // Airbrake drag
        let (cd_brake, a_brake) = airbrake_coeff_area(airbrake);
        let fd_brake = dynamic_pressure * cd_brake * a_brake;

        // Total drag (always opposes upward motion during coast)
        let fd = fd_body + fd_brake;

        let acc = -G - fd / MASS;
        let v_next = v + acc * DT;
        let x_next = x + v * DT + 0.5 * acc * DT * DT;

        if v_next < 0.0 {
            return x_next;
        }

        v = v_next;
        x = x_next;
    }
}
