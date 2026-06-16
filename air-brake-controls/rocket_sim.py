import math

# --------------------
# Rocket body aerodynamics (6-inch diameter)
# --------------------
BODY_CD = 0.4
BODY_DIAMETER = 0.1524                              # 6 inches in metres
BODY_AREA = math.pi * (BODY_DIAMETER / 2) ** 2     # ~0.01824 m²


def rocket_sim(x, v, tilt_deg, airbrake, ground_pressure, verbose=False):
    # --------------------
    # Constants
    # --------------------
    dt = 0.01          # time step (s)
    g = 9.80665        # gravity (m/s^2)
    R = 287.05         # J/(kg*K)
    L = 0.0065         # K/m
    mass = 113.0       # kg

    time = 0.0

    # --------------------
    # Ground conditions
    # --------------------
    T0 = 288.15        # K
    P0_pa = ground_pressure

    # --------------------
    # Helper functions
    # --------------------
    def air_density(h):
        """ISA air density — same formula as controller.py."""
        T = max(T0 - L * h, 1.0)
        P = P0_pa * (T / T0) ** (g / (R * L))
        return max(P / (R * T), 0.001)

    def airbrake_coeff_area(deploy):
        """Airbrake contribution only (body drag is added separately)."""
        Cd = 0.4
        A = 0.001848 + (0.021935 - 0.001848) * deploy  # 2.86479 in² to 34 in²
        return Cd, A

    # --------------------
    # Simulation loop
    # --------------------
    cos_tilt = max(math.cos(math.radians(abs(tilt_deg))), 1e-6)

    while True:
        # --- Air density at current altitude ---
        rho = air_density(x)

        # --- Airspeed along rocket axis ---
        v_air = abs(v) / cos_tilt

        # --- Body drag ---
        Fd_body = 0.5 * rho * v_air**2 * BODY_CD * BODY_AREA

        # --- Airbrake drag ---
        Cd_brake, A_brake = airbrake_coeff_area(airbrake)
        Fd_brake = 0.5 * rho * v_air**2 * Cd_brake * A_brake

        # --- Total drag (always opposes upward motion during coast) ---
        Fd = Fd_body + Fd_brake

        # --- Acceleration ---
        a = -g - Fd / mass

        # --- Kinematics ---
        v_next = v + a * dt
        x_next = x + v * dt + 0.5 * a * dt**2

        # --- Apogee condition ---
        if v_next < 0:
            if verbose:
                print(f"Apogee reached at {x:.2f} m after {time:.2f} s")
            break

        # --- Update ---
        v = v_next
        x = x_next
        time += dt
    return x_next


# --------------------
# CLI entry point
# --------------------
if __name__ == "__main__":
    def get_float(prompt, default):
        raw = input(f"{prompt} [default {default}]: ").strip()
        return float(raw) if raw else float(default)

    print("=== Rocket Coast Simulation ===")
    print("Press Enter to accept the default value.\n")

    while True:
        x0            = get_float("Initial altitude (m)",            0.0)
        v0            = get_float("Initial vertical velocity (m/s)", 200.0)
        tilt_deg      = get_float("Tilt off-axis (deg)",             0.0)
        airbrake      = get_float("Airbrake deployment level (0-1)", 0.0)
        ground_p_pa   = get_float("Ground pressure (Pa)",            101325.0)

        apogee = rocket_sim(x0, v0, tilt_deg, airbrake, ground_p_pa, verbose=True)
        print(f"  → Predicted apogee: {apogee:.2f} m\n")

        again = input("Run another simulation? (y/n) [n]: ").strip().lower()
        if again != "y":
            break
