"""
Airbrake Controller Architecture
Reads raw sensor data from BMP390 barometer and ICM-42688-P gyroscope.
Manages airbrake deployment using PID control and rocket simulation for apogee prediction.

The controller receives flight phase ("boost" or "coast") from the flight computer / simulator,
rather than detecting phases internally. This mirrors the real rocket architecture where
the flight computer determines state and passes it to the airbrake controller.
"""

import csv
import math
from collections import deque
from PID import PID
from rocket_sim import rocket_sim

# -----------------------------------------------------------------------------
# Constants
# -----------------------------------------------------------------------------
DT = 0.01  # Time step (s)
TARGET_APOGEE = 3048.0  # Target apogee in meters (adjust as needed)

# Atmospheric constants
R = 287.05  # Specific gas constant for dry air (J/(kg·K))
G = 9.80665  # Gravity (m/s^2)
L = 0.0065  # Temperature lapse rate (K/m) - Metric for how T changes with Height

# Ground conditions (auto-calibrated from first barometer reading)
GROUND_TEMP_K = 288.15  # Ground temperature (K) - standard sea level

# Airbrake limits
AIRBRAKE_MIN = 0.0
AIRBRAKE_MAX = 1.0
AIRBRAKE_RETRACT_STEP = 0.05  # Small step for adjustment loop

# Rocket mass
MASS = 113.0  # kg

# Rocket body aerodynamics (6-inch diameter)
BODY_CD = 0.4
BODY_DIAMETER = 0.1524       # 6 inches in meters
BODY_AREA = math.pi * (BODY_DIAMETER / 2) ** 2  # 0.01824 m²

# Airbrake aerodynamic characteristics
# Cd is constant at 0.4 across all deployments.
# Area varies linearly with deployment:
#   Deployment 0.0 -> Area = 2.86479 in² = 0.001848 m²
#   Deployment 1.0 -> Area = 34 in²      = 0.021935 m²
AIRBRAKE_CD = 0.4            # Constant drag coefficient
AIRBRAKE_AREA_MIN = 0.001848   # Area at 0% deployment (m²) — 2.86479 in²
AIRBRAKE_AREA_MAX = 0.021935   # Area at 100% deployment (m²) — 34 in²

# Failsafe thresholds
MAX_TILT_DEG = 50.0

# PID tuning parameters
KP = 0.008   # Proportional gain
KI = 0.0002  # Integral gain
KD = 0.001   # Derivative gain

# Converts PID apogee error (m) to a drag force adjustment (N)
DRAG_SCALE_FACTOR = 5.0


# -----------------------------------------------------------------------------
# Sensor Conversion Functions
# -----------------------------------------------------------------------------

def pressure_to_altitude(pressure_pa, P0, T0=GROUND_TEMP_K):
    """
    Convert BMP390 pressure reading to altitude using barometric formula.

    Args:
        pressure_pa: Pressure in Pascals from BMP390
        P0: Ground-level pressure in Pascals (calibrate before launch)
        T0: Ground-level temperature in Kelvin

    Returns:
        Altitude in meters above ground level
    """
    if pressure_pa <= 0:
        return 0.0

    # Barometric formula: h = (T0/L) * (1 - (P/P0)^(R*L/g))
    exponent = (R * L) / G
    altitude = (T0 / L) * (1.0 - (pressure_pa / P0) ** exponent)
    return max(0.0, altitude)


def gyro_to_angular_rate(gyro_raw, scale_factor=131.0):
    """
    Convert ICM-42688-P raw gyroscope reading to degrees per second.

    The ICM-42688-P outputs raw values that need scaling based on configured range.
    Default scale factors for different ranges:
        ±250 dps:  131 LSB/dps
        ±500 dps:  65.5 LSB/dps
        ±1000 dps: 32.8 LSB/dps
        ±2000 dps: 16.4 LSB/dps

    Args:
        gyro_raw: Raw gyroscope value from sensor
        scale_factor: LSB per degree/second (depends on configured range)

    Returns:
        Angular rate in degrees per second
    """
    return gyro_raw / scale_factor


# -----------------------------------------------------------------------------
# Aerodynamic Functions
# -----------------------------------------------------------------------------

def altitude_to_temperature(altitude, T0=GROUND_TEMP_K):
    """
    Calculate temperature at altitude using ISA lapse rate.

    Args:
        altitude: Altitude in meters
        T0: Ground-level temperature in Kelvin

    Returns:
        Temperature in Kelvin
    """
    T = T0 - L * altitude
    return max(T, 1.0)  # Prevent non-positive temperature


def air_density(altitude, P0, T0=GROUND_TEMP_K):
    """
    Calculate air density at a given altitude using ISA model.

    Args:
        altitude: Altitude in meters
        P0: Ground-level pressure in Pascals
        T0: Ground-level temperature in Kelvin

    Returns:
        Air density in kg/m³
    """
    T = altitude_to_temperature(altitude, T0)

    # Pressure at altitude
    P = P0 * (T / T0) ** (G / (R * L))

    # Density from ideal gas law: rho = P / (R * T)
    rho = P / (R * T)
    return max(rho, 0.001)  # Minimum density to prevent issues


def deployment_to_area(deployment):
    """
    Calculate airbrake frontal area as function of deployment.

    Linear interpolation:
        0% deployment -> 0.01 m²
        100% deployment -> 0.05 m²

    Args:
        deployment: Airbrake deployment (0.0 to 1.0)

    Returns:
        Frontal area in m²
    """
    # A(d) = A_min + (A_max - A_min) * d = 0.01 + 0.04*d
    return AIRBRAKE_AREA_MIN + (AIRBRAKE_AREA_MAX - AIRBRAKE_AREA_MIN) * deployment


def deployment_to_cd(deployment):
    """
    Return drag coefficient (constant at 0.3 for all deployments).

    Args:
        deployment: Airbrake deployment (0.0 to 1.0)

    Returns:
        Drag coefficient (dimensionless)
    """
    return AIRBRAKE_CD


def deployment_to_drag(deployment, velocity, altitude, ground_pressure):
    """
    Calculate drag force for a given deployment, velocity, and altitude.

    Drag equation: F_d = 0.5 * rho * v² * Cd * A

    Args:
        deployment: Airbrake deployment (0.0 to 1.0)
        velocity: Velocity in m/s
        altitude: Altitude in meters
        ground_pressure: Ground-level pressure in Pascals

    Returns:
        Drag force in Newtons
    """
    rho = air_density(altitude, ground_pressure)
    Cd = deployment_to_cd(deployment)
    A = deployment_to_area(deployment)

    return 0.5 * rho * velocity**2 * Cd * A


def drag_force_to_deployment(drag_force_n, velocity_mps, altitude_m, ground_pressure):
    """
    Convert desired drag force to airbrake deployment percentage.

    Solves the drag equation for deployment:
        F_d = 0.5 * rho * v² * Cd * A(d)

    With constant Cd and A(d) = A_min + (A_max - A_min)*d, this is linear in d:
        d = (F_d / (k * Cd) - A_min) / (A_max - A_min)   where k = 0.5 * rho * v²

    Args:
        drag_force_n: Desired drag force in Newtons
        velocity_mps: Current velocity in m/s
        altitude_m: Current altitude in meters
        ground_pressure: Ground-level pressure in Pascals

    Returns:
        Required deployment percentage (0.0 to 1.0), clamped to valid range
    """
    if velocity_mps <= 0:
        return AIRBRAKE_MIN

    rho = air_density(altitude_m, ground_pressure)
    k = 0.5 * rho * velocity_mps**2

    if k <= 0:
        return AIRBRAKE_MIN

    deployment = (drag_force_n / (k * AIRBRAKE_CD) - AIRBRAKE_AREA_MIN) / (AIRBRAKE_AREA_MAX - AIRBRAKE_AREA_MIN)

    return max(AIRBRAKE_MIN, min(AIRBRAKE_MAX, deployment))


# -----------------------------------------------------------------------------
# Sensor Buffer
# -----------------------------------------------------------------------------

class SensorBuffer:
    """Keeps last N sensor readings for calculating derivatives."""

    def __init__(self, size=3):
        self.altitudes = deque(maxlen=size)
        self.gyro_readings = deque(maxlen=size)
        self.timestamps = deque(maxlen=size)

    def add(self, altitude, gyro, timestamp):
        self.altitudes.append(altitude)
        self.gyro_readings.append(gyro)
        self.timestamps.append(timestamp)

    def is_ready(self):
        """Check if we have enough data points (3 for acceleration calc)."""
        return len(self.altitudes) >= 3

    def get_velocity(self):
        """Calculate velocity from altitude differences."""
        if len(self.altitudes) < 2:
            return 0.0
        # Use most recent two points
        dh = self.altitudes[-1] - self.altitudes[-2]
        dt = self.timestamps[-1] - self.timestamps[-2]
        return dh / dt if dt > 0 else 0.0

    def get_acceleration(self):
        """Calculate acceleration from velocity differences (second derivative of altitude)."""
        if len(self.altitudes) < 3:
            return 0.0

        dt1 = self.timestamps[-2] - self.timestamps[-3]
        dt2 = self.timestamps[-1] - self.timestamps[-2]

        if dt1 <= 0 or dt2 <= 0:
            return 0.0

        # v1 between points 0 and 1
        v1 = (self.altitudes[-2] - self.altitudes[-3]) / dt1
        # v2 between points 1 and 2
        v2 = (self.altitudes[-1] - self.altitudes[-2]) / dt2

        # Average dt for acceleration calculation
        dt_avg = (dt1 + dt2) / 2.0
        return (v2 - v1) / dt_avg


# -----------------------------------------------------------------------------
# Main Controller
# -----------------------------------------------------------------------------

class AirbrakeController:
    """
    Main controller for airbrake system.

    Receives flight phase from the flight computer / simulator:
        "boost" — motor burning, integrate gyroscope for tilt estimation
        "coast" — motor burned out, active PID airbrake control

    The controller does NOT detect phases internally; it trusts the
    passed-in phase, just like the real flight computer architecture.
    """

    def __init__(self, target_apogee=TARGET_APOGEE, ground_pressure=None,
                 ground_temp=GROUND_TEMP_K):
        self.target_apogee = target_apogee
        self.ground_pressure = ground_pressure  # None = auto-calibrate from first reading
        self.ground_temp = ground_temp
        self.sensor_buffer = SensorBuffer(size=3)

        # PID output can be positive (retract) or negative (deploy more).
        # Deployment clamping is handled inside calculate_drag_adjustment.
        self.pid = PID(KP, KI, KD, output_limits=(-AIRBRAKE_MAX, AIRBRAKE_MAX))

        # State variables
        self.current_airbrake = 0.0
        self.integrated_tilt_x = 0.0  # Accumulated tilt around X-axis (degrees)
        self.integrated_tilt_y = 0.0  # Accumulated tilt around Y-axis (degrees)
        self.integrated_tilt = 0.0  # Total tilt magnitude (degrees)
        self.coast_initialized = False  # True after first coast step initializes control
        self.connection_lost = False
        self.last_data_time = 0.0
        self._previous_time = None  # For dt calculation in step()

    def step(self, sensor_data):
        """
        Process one sensor reading and return current airbrake deployment.

        Args:
            sensor_data: dict with keys:
                time, pressure, gyro_x, gyro_y, gyro_z,
                phase ("boost" or "coast")

        Returns:
            Current airbrake deployment (0.0 to 1.0)
        """
        current_time = sensor_data['time']
        phase = sensor_data['phase']

        # Auto-calibrate ground pressure from first reading
        if self.ground_pressure is None:
            self.ground_pressure = sensor_data['pressure']
            print(f"[{current_time:.2f}s] Ground pressure calibrated: {self.ground_pressure:.1f} Pa")

        if self._previous_time is not None:
            dt = current_time - self._previous_time
        else:
            dt = DT
        self._previous_time = current_time
        self.last_data_time = current_time

        processed = self.process_sensors(sensor_data)
        altitude = processed['altitude']

        self.sensor_buffer.add(
            altitude,
            (processed['gyro_x'], processed['gyro_y'], processed['gyro_z']),
            current_time
        )

        # --- Boost phase: integrate gyro for tilt, no airbrake control ---
        if phase == "boost":
            self.integrate_gyroscope(processed['gyro_x'], processed['gyro_y'], dt)
            return self.current_airbrake

        # --- Coast phase: active airbrake control ---
        if phase == "coast":
            self.integrate_gyroscope(processed['gyro_x'], processed['gyro_y'], dt)

            # First coast step: initialize control
            if not self.coast_initialized:
                if self.sensor_buffer.is_ready():
                    self.coast_initialized = True
                    height = altitude
                    velocity = self.sensor_buffer.get_velocity()
                    tilt = self.integrated_tilt

                    predicted_apogee = rocket_sim(height, velocity, tilt, 0.0, self.ground_pressure)
                    print(f"[{current_time:.2f}s] Predicted apogee (no brakes): {predicted_apogee:.1f} m")

                    if predicted_apogee <= self.target_apogee:
                        print(f"[{current_time:.2f}s] Predicted apogee too low. Not deploying brakes.")
                        self.current_airbrake = AIRBRAKE_MIN
                    else:
                        self.current_airbrake = self.airbrake_adjustment_loop(height, velocity, tilt)
                        print(f"[{current_time:.2f}s] Initial airbrake deployment: {self.current_airbrake:.1%}")
                        self.command_airbrakes(self.current_airbrake)
                return self.current_airbrake

            # Active coast control
            height = altitude
            velocity = self.sensor_buffer.get_velocity()
            tilt = self.integrated_tilt

            if velocity <= 0:
                print(f"[{current_time:.2f}s] Apogee detected by controller at {height:.1f} m. Retracting.")
                self.current_airbrake = AIRBRAKE_MIN
                self.command_airbrakes(self.current_airbrake)
                return self.current_airbrake

            should_retract, message = self.check_failsafes(velocity)
            if should_retract:
                print(f"[{current_time:.2f}s] {message}")
                self.current_airbrake = AIRBRAKE_MIN
                self.command_airbrakes(self.current_airbrake)
                return self.current_airbrake

            predicted_apogee = rocket_sim(height, velocity, tilt, self.current_airbrake, self.ground_pressure)
            pid_output = self.pid.update(self.target_apogee, predicted_apogee, dt)
            self.current_airbrake = self.calculate_drag_adjustment(
                pid_output, velocity, height, self.current_airbrake
            )
            self.command_airbrakes(self.current_airbrake)

        return self.current_airbrake

    def process_sensors(self, sensor_data):
        """
        Convert raw sensor data to usable values.

        Returns:
            dict with 'altitude' and gyro rates in deg/s
        """
        # Convert pressure to altitude
        altitude = pressure_to_altitude(
            sensor_data['pressure'],
            P0=self.ground_pressure,
            T0=self.ground_temp
        )

        # Gyro data - assuming already in deg/s from CSV
        # If raw, uncomment the gyro_to_angular_rate conversion
        gyro_x = sensor_data['gyro_x']  # gyro_to_angular_rate(sensor_data['gyro_x'])
        gyro_y = sensor_data['gyro_y']  # gyro_to_angular_rate(sensor_data['gyro_y'])
        gyro_z = sensor_data['gyro_z']  # gyro_to_angular_rate(sensor_data['gyro_z'])

        return {
            'altitude': altitude,
            'gyro_x': gyro_x,
            'gyro_y': gyro_y,
            'gyro_z': gyro_z
        }

    def integrate_gyroscope(self, gyro_x, gyro_y, dt):
        """
        Integrate gyroscope readings to find tilt off-axis.
        Integrates X and Y components separately, then computes total tilt
        as the magnitude. This allows tilt to decrease if the rocket
        oscillates back toward vertical.

        Args:
            gyro_x: Angular rate around X-axis (deg/s)
            gyro_y: Angular rate around Y-axis (deg/s)
            dt: Time step in seconds

        Returns:
            Integrated tilt angle in degrees
        """
        self.integrated_tilt_x += gyro_x * dt
        self.integrated_tilt_y += gyro_y * dt
        self.integrated_tilt = math.sqrt(self.integrated_tilt_x**2 + self.integrated_tilt_y**2)
        return self.integrated_tilt

    def check_failsafes(self, current_velocity):
        """
        Check failsafe conditions.

        Returns: (should_retract, message)
        """
        # Failsafe 1: Excessive tilt
        if self.integrated_tilt > MAX_TILT_DEG:
            return True, f"FAILSAFE: Tilt {self.integrated_tilt:.1f}° exceeds {MAX_TILT_DEG}°. Fully retracting."

        # Failsafe 2: Connection lost (no new data but velocity > 0)
        if self.connection_lost and current_velocity > 0:
            return True, "FAILSAFE: Connection lost with positive velocity. Fully retracting."

        return False, None

    def airbrake_adjustment_loop(self, height, velocity, tilt):
        """
        Binary search for the minimum deployment where predicted apogee >= target.

        Each iteration halves the search interval; 20 iterations gives
        precision of ~0.0001 (0.01% deployment).

        Returns the optimal airbrake deployment level.
        """
        lo = AIRBRAKE_MIN
        hi = AIRBRAKE_MAX

        for _ in range(20):
            mid = (lo + hi) / 2.0
            predicted_apogee = rocket_sim(height, velocity, tilt, mid, self.ground_pressure)
            if predicted_apogee >= self.target_apogee:
                lo = mid   # this deployment still undershoots drag, can add more
            else:
                hi = mid   # too much drag, back off

        # lo is the highest deployment where predicted apogee just meets target
        return min(lo, AIRBRAKE_MAX)

    def calculate_drag_adjustment(self, pid_output, current_velocity, current_altitude, current_deployment):
        """
        Convert PID output to airbrake deployment using drag force model.

        The PID outputs a value based on apogee error:
        - Positive output = predicted apogee < target, need less drag (retract)
        - Negative output = predicted apogee > target, need more drag (deploy)

        This method:
        1. Calculates current drag force from current deployment
        2. Adjusts drag force based on PID output (scaled appropriately)
        3. Converts target drag force back to deployment percentage

        Args:
            pid_output: Output from PID controller
            current_velocity: Current velocity in m/s
            current_altitude: Current altitude in m
            current_deployment: Current airbrake deployment (0.0 to 1.0)

        Returns:
            New deployment percentage (0.0 to 1.0)
        """
        # Calculate current drag force
        current_drag = deployment_to_drag(current_deployment, current_velocity, current_altitude, self.ground_pressure)

        # Scale PID output to drag force adjustment
        # PID output is based on apogee error (meters)
        # We need to convert this to a drag force change
        # Positive PID = apogee too low = reduce drag
        # Negative PID = apogee too high = increase drag
        target_drag = current_drag - (pid_output * DRAG_SCALE_FACTOR)

        # Ensure target drag is non-negative
        target_drag = max(0.0, target_drag)

        # Convert target drag force to deployment percentage
        new_deployment = drag_force_to_deployment(target_drag, current_velocity, current_altitude, self.ground_pressure)

        return new_deployment

    def command_airbrakes(self, deployment):
        """
        Send deployment command to airbrake actuators.
        Override this method for actual hardware interface.

        Args:
            deployment: Deployment percentage (0.0 to 1.0)
        """
        # Clamp value
        deployment = max(AIRBRAKE_MIN, min(AIRBRAKE_MAX, deployment))
        self.current_airbrake = deployment
        # In actual implementation: send command to actuators


# -----------------------------------------------------------------------------
# Entry Points
# -----------------------------------------------------------------------------

def run_from_csv(csv_path, target_apogee=TARGET_APOGEE,
                 ground_pressure=None, ground_temp=GROUND_TEMP_K):
    """
    Convenience function to run controller from CSV file.
    CSV must include a 'phase' column ("boost" or "coast").

    Args:
        csv_path: Path to sensor data CSV
        target_apogee: Target apogee in meters
        ground_pressure: Ground pressure in Pa (None = auto-calibrate from first reading)
        ground_temp: Ground temperature in Kelvin
    """
    controller = AirbrakeController(
        target_apogee=target_apogee,
        ground_pressure=ground_pressure,
        ground_temp=ground_temp
    )

    print("Airbrake Controller Starting...")
    print(f"Target Apogee: {target_apogee} m")
    print(f"Ground Pressure: {'auto-calibrate' if ground_pressure is None else f'{ground_pressure} Pa'}")
    print(f"Ground Temperature: {ground_temp} K")

    with open(csv_path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            sensor_data = {
                'time': float(row.get('time', 0)),
                'pressure': float(row.get('pressure', GROUND_PRESSURE_PA)),
                'gyro_x': float(row.get('gyro_x', 0)),
                'gyro_y': float(row.get('gyro_y', 0)),
                'gyro_z': float(row.get('gyro_z', 0)),
                'phase': row.get('phase', 'boost'),
            }
            controller.step(sensor_data)

    print("Controller finished.")
    return controller.current_airbrake


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("Usage: python controller.py <sensor_data.csv> [target_apogee] [ground_pressure] [ground_temp]")
        print()
        print("Expected CSV columns:")
        print("  time     - Timestamp in seconds")
        print("  pressure - BMP390 pressure in Pascals")
        print("  gyro_x   - ICM-42688-P X-axis angular rate (deg/s)")
        print("  gyro_y   - ICM-42688-P Y-axis angular rate (deg/s)")
        print("  gyro_z   - ICM-42688-P Z-axis angular rate (deg/s)")
        print("  phase    - Flight phase: 'boost' or 'coast'")
        sys.exit(1)

    csv_file = sys.argv[1]
    target = float(sys.argv[2]) if len(sys.argv) > 2 else TARGET_APOGEE
    pressure = float(sys.argv[3]) if len(sys.argv) > 3 else None
    temp = float(sys.argv[4]) if len(sys.argv) > 4 else GROUND_TEMP_K

    run_from_csv(csv_file, target, pressure, temp)
