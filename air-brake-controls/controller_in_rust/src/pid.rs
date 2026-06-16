/// PID controller with output clamping. D term skipped on first call.
pub struct Pid {
    kp: f64,
    ki: f64,
    kd: f64,
    min_output: f64,
    max_output: f64,
    integral: f64,
    prev_error: f64,
    first_call: bool,
}

impl Pid {
    pub fn new(kp: f64, ki: f64, kd: f64, min_output: f64, max_output: f64) -> Self {
        Pid {
            kp,
            ki,
            kd,
            min_output,
            max_output,
            integral: 0.0,
            prev_error: 0.0,
            first_call: true,
        }
    }

    /// Compute clamped P+I+D output. error = setpoint − measurement.
    pub fn update(&mut self, setpoint: f64, measurement: f64, dt: f64) -> f64 {
        let error = setpoint - measurement;

        let p = self.kp * error;

        self.integral += error * dt;
        let i = self.ki * self.integral;

        let d = if self.first_call {
            self.first_call = false;
            0.0
        } else {
            let derivative = (error - self.prev_error) / dt;
            self.kd * derivative
        };

        self.prev_error = error;

        (p + i + d).clamp(self.min_output, self.max_output)
    }
}
