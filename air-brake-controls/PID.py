class PID:
    def __init__(self, Kp, Ki, Kd, output_limits=(None, None)):
        """
        PID controller

        Args:
            Kp, Ki, Kd : floats, PID gains
            output_limits : (min, max), saturation limits for control output
        """
        self.Kp = Kp
        self.Ki = Ki
        self.Kd = Kd
        self.min_output, self.max_output = output_limits

        self.integral = 0.0
        self.prev_error = 0.0
        self.first_call = True

    def update(self, setpoint, measurement, dt):
        """
        Compute PID output

        Args:
            setpoint : desired target value
            measurement : current measured value
            dt : timestep

        Returns:
            control_output : float
        """
        error = setpoint - measurement

        # Proportional
        P = self.Kp * error

        # Integral
        self.integral += error * dt
        I = self.Ki * self.integral

        # Derivative (guard against first call)
        if self.first_call:
            D = 0.0
            self.first_call = False
        else:
            derivative = (error - self.prev_error) / dt
            D = self.Kd * derivative

        # Save error
        self.prev_error = error

        # PID sum
        output = P + I + D

        # Apply output limits
        if self.min_output is not None:
            output = max(self.min_output, output)
        if self.max_output is not None:
            output = min(self.max_output, output)

        return output
