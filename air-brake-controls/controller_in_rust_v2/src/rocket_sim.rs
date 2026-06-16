use num_traits::Float;

// Rocket body aerodynamics (6-inch diameter)
pub const BODY_CD: f32 = 0.5;
pub const BODY_DIAMETER: f32 = 0.1524;                                       // 6 inches in metres
pub const BODY_AREA: f32 = core::f32::consts::PI * (BODY_DIAMETER / 2.0) * (BODY_DIAMETER / 2.0); // ~0.01824 m²

/// Simulates coast phase from given conditions and returns predicted apogee (m).
pub fn rocket_sim(x: f32, v: f32, tilt_deg: f32, airbrake: f32, ground_pressure: f32) -> f32 {
    const DT: f32 = 0.01;
    const G: f32 = 9.80665;
    const R: f32 = 287.05;
    const L: f32 = 0.0065;
    const MASS: f32 = 51.26; // kg (113 lb)
    const T0: f32 = 288.15;
    let p0_pa = ground_pressure;

    // ISA air density — same formula as controller.rs
    let air_density = |h: f32| -> f32 {
        let t = (T0 - L * h).max(1.0);
        let p = p0_pa * libm::powf(t / T0, G / (R * L));
        (p / (R * t)).max(0.001)
    };

    // Airbrake contribution only (body drag is added separately)
    let airbrake_coeff_area = |deploy: f32| -> (f32, f32) {
        (0.3, 0.001848 + (0.021935 - 0.001848) * deploy) // 2.86479 in² to 34 in²
    };

    let cos_tilt = libm::cosf(libm::fabsf(tilt_deg).to_radians()).max(1e-6);

    let mut x = x;
    let mut v = v;

    loop {
        let rho = air_density(x);

        let v_air = libm::fabsf(v) / cos_tilt;
        let dynamic_pressure = 0.5 * rho * v_air * v_air;

        let fd_body = dynamic_pressure * BODY_CD * BODY_AREA;

        let (cd_brake, a_brake) = airbrake_coeff_area(airbrake);
        let fd_brake = dynamic_pressure * cd_brake * a_brake;

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
