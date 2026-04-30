"""
Simulated IMU for airbrake controller testing.

Replays burn_data.csv through the controller (boost phase),
then simulates coast phase with synthetic BMP390/ICM-42688-P sensor data
(Gaussian noise matched to datasheet specs) until apogee.

The simulator passes the flight phase ("boost" or "coast") to the controller
in each sensor packet, mirroring the real rocket architecture where the
flight computer determines state.

Usage:
    python simulated_imu.py                          # uses burn_data.csv, default target
    python simulated_imu.py burn_data.csv            # explicit CSV path
    python simulated_imu.py burn_data.csv 3048       # with target apogee (m)

Sensor noise specs:
    ICM-42688-P gyroscope: 0.07 deg/s RMS
    BMP390 barometer:      0.02 Pa RMS
"""

import csv
import math
import random
import re
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from controller import (
    AirbrakeController, TARGET_APOGEE,
    GROUND_TEMP_K,
    DT, G, L, R,
    AIRBRAKE_CD, AIRBRAKE_AREA_MIN, AIRBRAKE_AREA_MAX,
    air_density, deployment_to_area
)
from rocket_sim import rocket_sim

# -----------------------------------------------------------------------------
# Sensor noise (datasheet values)
# -----------------------------------------------------------------------------
GYRO_NOISE_STD  = 0.07   # deg/s RMS  (ICM-42688-P)
BARO_NOISE_STD  = 0.02   # Pa RMS     (BMP390)

# Rocket mass (must match rocket_sim.py)
MASS = 113.0  # kg


# -----------------------------------------------------------------------------
# Helper
# -----------------------------------------------------------------------------

def next_run_path(script_dir):
    """
    Returns the path for the next auto-incremented run CSV inside simulated_flights/.
    Files are named run_001.csv, run_002.csv, etc.
    """
    out_dir = os.path.join(script_dir, "successful_runs")
    os.makedirs(out_dir, exist_ok=True)

    existing = [
        int(m.group(1))
        for f in os.listdir(out_dir)
        if (m := re.fullmatch(r"run_(\d+)\.csv", f))
    ]
    run_num = max(existing, default=0) + 1
    return os.path.join(out_dir, f"run_{run_num:03d}.csv")


def altitude_to_pressure(altitude, P0, T0=GROUND_TEMP_K):
    """
    Inverse of the barometric formula used in controller.py.
    Converts altitude (m) back to pressure (Pa).
    """
    T = T0 - L * altitude
    T = max(T, 1.0)
    return P0 * (T / T0) ** (G / (R * L))


# -----------------------------------------------------------------------------
# Main simulation
# -----------------------------------------------------------------------------

def run_simulation(burn_csv_path, target_apogee=TARGET_APOGEE):
    """
    Run full flight simulation:
      Phase 1 — Replay burn_data.csv through controller (boost phase).
      Phase 2 — Simulate coast with synthetic IMU data until apogee.

    The simulator passes phase="boost" or phase="coast" in each sensor packet.

    Args:
        burn_csv_path: Path to burn_data.csv
        target_apogee: Target apogee in meters

    Returns:
        Achieved apogee altitude in meters
    """
    controller = AirbrakeController(target_apogee=target_apogee)
    rows = []  # accumulated CSV rows

    print("=" * 60)
    print("SIMULATED IMU FLIGHT SIMULATION")
    print("=" * 60)
    print(f"Burn data:     {burn_csv_path}")
    print(f"Target apogee: {target_apogee:.1f} m")
    print("=" * 60)

    # Print table header once before burn phase
    print()
    print("-" * 88)
    print(f"{'Time':>7}  {'Phase':>6}  {'Alt':>7}  {'Vel':>7}  {'Deploy':>7}  {'Drag':>8}  {'PredApog':>9}  {'Error':>8}")
    print(f"{'(s)':>7}  {'':>6}  {'(m)':>7}  {'(m/s)':>7}  {'(%)':>7}  {'(N)':>8}  {'(m)':>9}  {'(m)':>8}")
    print("-" * 88)

    # -------------------------------------------------------------------------
    # Phase 1: Burn replay — pass phase="boost" to controller
    # -------------------------------------------------------------------------
    last_row = None
    with open(burn_csv_path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            sensor_data = {
                'time':     float(row['time']),
                'pressure': float(row['pressure']),
                'gyro_x':   float(row['gyro_x']),
                'gyro_y':   float(row['gyro_y']),
                'gyro_z':   float(row['gyro_z']),
                'phase':    'boost',
            }
            controller.step(sensor_data)
            last_row = sensor_data

            # Print burn state from sensor buffer (0% deployment during burn)
            buf_alt = controller.sensor_buffer.altitudes[-1] if controller.sensor_buffer.altitudes else 0.0
            buf_vel = controller.sensor_buffer.get_velocity()
            rho_burn = air_density(buf_alt, controller.ground_pressure)
            v_axial_burn = buf_vel / math.cos(math.radians(controller.integrated_tilt)) if buf_vel > 0 else 0.0
            drag_burn = 0.5 * rho_burn * v_axial_burn**2 * AIRBRAKE_CD * AIRBRAKE_AREA_MIN
            print(f"{sensor_data['time']:7.2f}  {'BOOST':>6}  {buf_alt:7.1f}  {buf_vel:7.1f}  {0.0:7.1f}  {drag_burn:8.2f}  {'---':>9}  {'---':>8}")
            rows.append({
                'time_s': sensor_data['time'],
                'phase': 'BOOST',
                'altitude_m': buf_alt,
                'velocity_m_s': buf_vel,
                'deployment_pct': 0.0,
                'drag_N': drag_burn,
                'predicted_apogee_m': '',
                'error_m': '',
            })

    if last_row is None:
        raise ValueError("burn_data.csv is empty.")

    # Extract burnout state from controller
    burnout_time     = last_row['time']
    burnout_altitude = controller.sensor_buffer.altitudes[-1]
    burnout_velocity = controller.sensor_buffer.get_velocity()
    burnout_tilt_deg = controller.integrated_tilt

    # -------------------------------------------------------------------------
    # Phase 2: Coast simulation — pass phase="coast" to controller
    # -------------------------------------------------------------------------

    # Physics state (ground truth)
    h          = burnout_altitude
    v          = burnout_velocity
    tilt_rad   = math.radians(burnout_tilt_deg)
    cos_tilt   = math.cos(tilt_rad)
    t          = burnout_time
    deployment = controller.current_airbrake
    apogee_h   = h

    while v > 0:
        # --- Generate synthetic sensor readings ---
        pressure_noisy = altitude_to_pressure(h, controller.ground_pressure) + random.gauss(0, BARO_NOISE_STD)
        gyro_x = random.gauss(0, GYRO_NOISE_STD)   # tilt fixed -> true rate = 0
        gyro_y = random.gauss(0, GYRO_NOISE_STD)
        gyro_z = random.gauss(0, GYRO_NOISE_STD)

        sensor_data = {
            'time':     t,
            'pressure': pressure_noisy,
            'gyro_x':   gyro_x,
            'gyro_y':   gyro_y,
            'gyro_z':   gyro_z,
            'phase':    'coast',
        }

        # --- Feed to controller, get deployment ---
        deployment = controller.step(sensor_data)

        # --- Physics: airspeed along rocket axis ---
        # v is vertical velocity (one leg); rocket axis is hypotenuse tilted at tilt_rad
        v_axial = v / cos_tilt

        # --- Drag along rocket axis ---
        rho    = air_density(h, controller.ground_pressure)
        A      = deployment_to_area(deployment)
        F_drag = 0.5 * rho * v_axial**2 * AIRBRAKE_CD * A

        # --- Project drag back to vertical ---
        F_drag_vertical = F_drag * cos_tilt

        # --- Vertical acceleration ---
        a = -G - F_drag_vertical / MASS

        # --- Integrate ---
        v += a * DT
        h += v * DT
        t += DT

        apogee_h = max(apogee_h, h)

        pred_apogee = rocket_sim(h, v, burnout_tilt_deg, deployment, controller.ground_pressure)
        error = pred_apogee - target_apogee
        print(f"{t:7.2f}  {'COAST':>6}  {h:7.1f}  {v:7.1f}  {deployment*100:7.1f}  {F_drag_vertical:8.2f}  {pred_apogee:9.1f}  {error:+8.1f}")
        rows.append({
            'time_s': t,
            'phase': 'COAST',
            'altitude_m': h,
            'velocity_m_s': v,
            'deployment_pct': deployment * 100,
            'drag_N': F_drag_vertical,
            'predicted_apogee_m': pred_apogee,
            'error_m': error,
        })

    print("-" * 60)
    print(f"\n[APOGEE]  t={t:.2f}s,  altitude={apogee_h:.1f} m")
    print(f"  Target:  {target_apogee:.1f} m")
    print(f"  Error:   {apogee_h - target_apogee:+.1f} m  "
          f"({(apogee_h / target_apogee - 1) * 100:+.2f}%)")
    print("=" * 60)

    # Write CSV output
    script_dir = os.path.dirname(os.path.abspath(__file__))
    csv_path = next_run_path(script_dir)
    fieldnames = ['time_s', 'phase', 'altitude_m', 'velocity_m_s',
                  'deployment_pct', 'drag_N', 'predicted_apogee_m', 'error_m']
    with open(csv_path, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)
    print(f"\nRun saved to: {csv_path}")

    return apogee_h


# -----------------------------------------------------------------------------
# Entry point
# -----------------------------------------------------------------------------

if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))

    burn_csv = os.path.join(script_dir, "comp_25_clean.csv")
    if len(sys.argv) > 1:
        burn_csv = sys.argv[1]

    if not os.path.exists(burn_csv):
        print(f"Error: '{burn_csv}' not found.")
        sys.exit(1)

    target = float(sys.argv[2]) if len(sys.argv) > 2 else TARGET_APOGEE

    run_simulation(burn_csv, target)
