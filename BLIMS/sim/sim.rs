/// sim.rs — BLiMS Closed-Loop Flight Simulation
///
/// Unlike a replay sim, this feeds the BLiMS controller's own motor commands
/// back into a parafoil physics model and integrates a new position each step.
/// The controller steers a simulated rocket, not the recorded one.
///
/// What is real (from the CSV):
///   - Barometric altitude    – phase transitions trigger at the right altitudes
///   - Timestamps             – dt is accurate, loiter alarms fire on schedule
///
/// What is simulated (physics model):
///   - Lat / Lon              – integrated from airspeed + wind each step
///   - Heading                – updated by turn rate driven by motor position
///   - Ground speed / NED vel – computed from the velocity vector
///
/// Output:
///   stdout  – TSV log, one row per iteration
///   stderr  – phase transitions + final landing position vs target
///
/// Tunable constants (top of file):
///   TARGET_LAT / TARGET_LON  – landing target
///   WIND_FROM_DEG            – wind direction (degrees FROM)
///   WIND_SPEED_MS            – wind speed (m/s)
///   AIRSPEED_MS              – parafoil forward airspeed through air (m/s)
///   SINK_RATE_MS             – vertical descent rate (m/s)
///   MAX_TURN_RATE_DEG_S      – turn rate at full brake deflection (deg/s)

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use src::blims::{BLIMS, Hardware};
use src::blims_state::BLIMSDataIn;

// ── Mission configuration ─────────────────────────────────────────────────────

const TARGET_LAT: f32    = 42.698011;   // Landing target, decimal degrees
const TARGET_LON: f32    = -77.1911059;

const WIND_FROM_DEG: f32 = 200.0;      // Wind blowing FROM this direction (0=N, 90=E)
const WIND_SPEED_MS: f32 = 3.0;        // Wind speed, m/s

// Parafoil aerodynamic constants.
// Derived from L3 Launch 4 telemetry: avg ground speed 7.2 m/s, sink 8.2 m/s.
// Adjust these to match your canopy's known performance.
const AIRSPEED_MS: f32         = 7.5;  // Forward airspeed through the air, m/s
const SINK_RATE_MS: f32        = 8.2;  // Vertical descent rate, m/s
const MAX_TURN_RATE_DEG_S: f32 = 15.0; // Turn rate at full brake (motor 0.3 or 0.7), deg/s

const CSV_PATH: &str = "sim/L3_Launch4_2025.csv";

// ── Local constants (mirror blims_constants.rs) ───────────────────────────────
// Defined here so the binary doesn't need to import the private constants crate.

const NEUTRAL_POS: f32 = 0.5;
const MOTOR_MAX:   f32 = 0.7;
const FT_PER_M:    f32 = 3.2808;
const DEG_TO_RAD:  f32 = std::f32::consts::PI / 180.0;
const WRAP_CYCLE_COUNT: u16 = 65535;
const ALT_NEUTRAL_FT:   f32 = 100.0;  // below this BLiMS goes hands-off

// ── Parafoil physics model ────────────────────────────────────────────────────

/// Simulated parafoil state.  Altitude is not stored here — it comes from the
/// CSV barometer, which is independent of horizontal position.
struct Parafoil {
    lat: f64,       // Current latitude,  decimal degrees
    lon: f64,       // Current longitude, decimal degrees
    heading: f32,   // Current heading,   degrees CW from North
}

impl Parafoil {
    fn new(lat: f64, lon: f64, heading: f32) -> Self {
        Self { lat, lon, heading }
    }

    /// Advance the parafoil one time step.
    ///
    /// Given the current motor position and elapsed time `dt` (seconds):
    ///   1. Convert motor deflection from neutral into a turn rate (deg/s).
    ///   2. Rotate the heading.
    ///   3. Build the airspeed vector (in the heading direction).
    ///   4. Add the wind vector to get ground velocity.
    ///   5. Integrate lat/lon from ground velocity.
    ///
    /// Returns `(ground_speed_ms, vel_north_ms, vel_east_ms)` so the caller
    /// can build a realistic `BLIMSDataIn` for the controller.
    fn step(
        &mut self,
        motor_pos: f32,
        wind_from_deg: f32,
        wind_speed_ms: f32,
        dt: f32,
    ) -> (f32, f32, f32) {
        // ── 1. Turn rate ──────────────────────────────────────────────────────
        // Motor 0.5 = neutral = 0 turn rate.
        // Motor 0.7 = full right brake = +MAX_TURN_RATE.
        // Motor 0.3 = full left  brake = -MAX_TURN_RATE.
        // Linear interpolation between neutral and the active limit.
        let deflection = (motor_pos - NEUTRAL_POS) / (MOTOR_MAX - NEUTRAL_POS); // -1 to +1
        let turn_rate  = deflection * MAX_TURN_RATE_DEG_S;                       // deg/s

        // ── 2. Update heading ─────────────────────────────────────────────────
        self.heading = wrap360(self.heading + turn_rate * dt);

        // ── 3. Airspeed vector (direction the canopy is pointing) ─────────────
        let hdg_rad   = self.heading * DEG_TO_RAD;
        let air_north = AIRSPEED_MS * hdg_rad.cos();
        let air_east  = AIRSPEED_MS * hdg_rad.sin();

        // ── 4. Wind vector (wind blows TO the opposite of FROM direction) ─────
        let wind_to_rad = wrap360(wind_from_deg + 180.0) * DEG_TO_RAD;
        let wind_north  = wind_speed_ms * wind_to_rad.cos();
        let wind_east   = wind_speed_ms * wind_to_rad.sin();

        // ── 5. Ground velocity = airspeed + wind ──────────────────────────────
        let gnd_north = air_north + wind_north;
        let gnd_east  = air_east  + wind_east;

        // ── 6. Integrate position ─────────────────────────────────────────────
        // Flat-earth approximation: accurate to < 0.1 % over distances < 5 km.
        let lat_rad   = (self.lat as f32) * DEG_TO_RAD;
        self.lat += (gnd_north * dt / 111_320.0)                     as f64;
        self.lon += (gnd_east  * dt / (111_320.0 * lat_rad.cos()))   as f64;

        let gnd_speed = libm_sqrtf(gnd_north * gnd_north + gnd_east * gnd_east);
        (gnd_speed, gnd_north, gnd_east)
    }
}

// Tiny sqrt wrapper so we don't need the libm crate in this binary.
fn libm_sqrtf(x: f32) -> f32 { (x as f64).sqrt() as f32 }

fn wrap360(mut a: f32) -> f32 {
    a %= 360.0;
    if a < 0.0 { a += 360.0; }
    a
}

// ── SimHardware ───────────────────────────────────────────────────────────────

struct SimHardware {
    time_ms:       u32,
    pwm_level:     u16,
    enable_high:   bool,
    alarm:         Option<(u32, i64)>,
    next_alarm_id: i64,
    pub alarm_fired: bool,
}

impl SimHardware {
    fn new() -> Self {
        Self {
            time_ms: 0, pwm_level: 0, enable_high: false,
            alarm: None, next_alarm_id: 0, alarm_fired: false,
        }
    }

    /// Advance the simulated clock and fire any pending alarm if its
    /// deadline has passed.
    fn advance_to_ms(&mut self, t: u32) {
        self.time_ms = t;
        if let Some((fire_at, _)) = self.alarm {
            if t >= fire_at {
                self.alarm       = None;
                self.alarm_fired = true;
            }
        }
    }
}

impl Hardware for SimHardware {
    fn now_ms(&self) -> u32 { self.time_ms }

    fn set_pwm_level(&mut self, _pin: u8, level: u16) { self.pwm_level = level; }

    fn set_enable_pin(&mut self, _pin: u8, high: bool) { self.enable_high = high; }

    fn schedule_alarm_ms(&mut self, delay_ms: u32) -> i64 {
        let id      = self.next_alarm_id;
        let fire_at = self.time_ms.saturating_add(delay_ms);
        self.alarm  = Some((fire_at, id));
        self.next_alarm_id += 1;
        id
    }

    fn cancel_alarm(&mut self, alarm_id: i64) {
        if let Some((_, id)) = self.alarm {
            if id == alarm_id { self.alarm = None; }
        }
        self.alarm_fired = false;
    }
}

// ── CSV row ───────────────────────────────────────────────────────────────────

/// One raw row from the CSV, in original units.
struct CsvRow {
    time_ms:    u32,
    altitude_m: f32,    // barometric AGL, metres
    gps_valid:  bool,
    num_sats:   u8,
    lat_raw:    i32,    // degrees × 1e7  (used only to seed initial position)
    lon_raw:    i32,
    head_raw:   i32,    // degrees × 1e5  (used only to seed initial heading)
}

fn col(header: &[&str], name: &str) -> usize {
    header.iter().position(|h| h.trim() == name)
          .unwrap_or_else(|| panic!("CSV missing column: {name}"))
}

fn load_csv(path: &str) -> Vec<CsvRow> {
    let file   = File::open(path).unwrap_or_else(|e| panic!("Cannot open {path}: {e}"));
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header_line = lines.next().expect("empty").expect("io error");
    let header: Vec<&str> = header_line.split(',').collect();

    let ci_ts   = col(&header, "Timestamp");
    let ci_alt  = col(&header, "Altitude");
    let ci_gps  = col(&header, "GPS_Status");
    let ci_sats = col(&header, "Number_of_Satellites");
    let ci_lat  = col(&header, "Latitude");
    let ci_lon  = col(&header, "Longitude");
    let ci_hm   = col(&header, "Heading_of_Motion");

    let mut rows = Vec::new();
    for line_result in lines {
        let line = match line_result { Ok(l) => l, Err(_) => continue };
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 20 { continue; }

        let g = |i: usize| f.get(i).copied().unwrap_or("").trim();
        let pf32 = |i: usize| -> f32 { g(i).parse().unwrap_or(0.0) };
        let pf64 = |i: usize| -> f64 { g(i).parse().unwrap_or(0.0) };
        let pu8  = |i: usize| -> u8  { g(i).parse().unwrap_or(0)   };
        let pi32 = |i: usize| -> i32 { g(i).parse().unwrap_or(0)   };

        rows.push(CsvRow {
            time_ms:  (pf64(ci_ts) * 1000.0).round() as u32,
            altitude_m: pf32(ci_alt),
            gps_valid:  pu8(ci_gps) == 1,
            num_sats:   pu8(ci_sats),
            lat_raw:    pi32(ci_lat),
            lon_raw:    pi32(ci_lon),
            head_raw:   pf64(ci_hm).round() as i32,
        });
    }
    rows
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn phase_name(phase_id: i8) -> &'static str {
    match phase_id {
        0 => "Held",
        1 => "InitialHold",
        2 => "Upwind",
        3 => "Downwind",
        4 => "Neutral",
        _ => "?",
    }
}

/// Great-circle distance between two lat/lon points, metres.
fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = (lat2 - lat1) as f32;
    let d_lon = (lon2 - lon1) as f32;
    let lat_r = (lat1 as f32) * DEG_TO_RAD;
    let dn = d_lat * 111_320.0;
    let de = d_lon * 111_320.0 * lat_r.cos();
    libm_sqrtf(dn * dn + de * de) as f64
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    eprintln!("Loading {}…", CSV_PATH);
    let rows = load_csv(CSV_PATH);
    eprintln!("Loaded {} rows.", rows.len());

    // ── Seed initial parafoil position and heading ────────────────────────────
    // Use the first GPS-valid row at altitude so the physics starts from a
    // realistic position (the parafoil has deployed and is in freefall).
    let seed = rows.iter()
        .find(|r| r.gps_valid && r.num_sats >= 4 && r.altitude_m > 100.0)
        .unwrap_or(&rows[0]);

    let init_lat     = seed.lat_raw  as f64 * 1e-7;
    let init_lon     = seed.lon_raw  as f64 * 1e-7;
    let init_heading = seed.head_raw as f32 * 1e-5;

    let mut parafoil = Parafoil::new(init_lat, init_lon, init_heading);

    // Keep the recorded landing position for comparison
    let recorded_final = rows.last().unwrap();
    let recorded_lat = recorded_final.lat_raw as f64 * 1e-7;
    let recorded_lon = recorded_final.lon_raw as f64 * 1e-7;

    // ── Initialise BLiMS ──────────────────────────────────────────────────────
    let mut hw    = SimHardware::new();
    let mut blims = BLIMS::new();
    blims.begin(&mut hw, 0, 1);
    blims.set_target(TARGET_LAT, TARGET_LON);
    blims.set_wind_from_deg(WIND_FROM_DEG);

    eprintln!("Target            : {TARGET_LAT:.6}°N  {TARGET_LON:.6}°E");
    eprintln!("Wind              : {WIND_SPEED_MS:.1} m/s FROM {WIND_FROM_DEG:.0}°");
    eprintln!("Parafoil airspeed : {AIRSPEED_MS:.1} m/s   sink {SINK_RATE_MS:.1} m/s");
    eprintln!("Init position     : {init_lat:.6}°N  {init_lon:.6}°E  hdg {init_heading:.1}°");
    eprintln!("{}", "─".repeat(72));

    // ── TSV header ────────────────────────────────────────────────────────────
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    writeln!(out,
        "Time_ms\tAlt_m\tAlt_ft\t\
         Sim_Lat\tSim_Lon\t\
         Sim_Heading\tBearing_to_Target\t\
         GndSpd_ms\tPhase\tMotorPos\tPID_P\tPID_I\tLoiterStep\t\
         Dist_to_Target_m"
    ).unwrap();

    // ── Simulation loop ───────────────────────────────────────────────────────
    let mut last_phase:      i8  = -1;
    let mut prev_time_ms:    u32 = 0;
    let mut motor_pos:       f32 = NEUTRAL_POS; // motor command from previous step

    // Track landing position (last row where altitude is above ground)
    let mut landing_lat = init_lat;
    let mut landing_lon = init_lon;
    let mut landed      = false;

    // Phase-entry altitude table
    let mut phase_entries: Vec<(i8, f32)> = Vec::new();

    for (i, row) in rows.iter().enumerate() {

        // ── 1. Advance hardware clock, fire alarms ────────────────────────────
        hw.advance_to_ms(row.time_ms);
        if hw.alarm_fired {
            hw.alarm_fired = false;
            blims.notify_loiter_alarm();
        }

        // ── 2. Compute dt ─────────────────────────────────────────────────────
        let dt = if i == 0 {
            0.05 // assume 50 ms for the very first step
        } else {
            let delta = row.time_ms.saturating_sub(prev_time_ms) as f32 / 1000.0;
            delta.max(0.001) // guard against duplicate timestamps
        };
        prev_time_ms = row.time_ms;

        // ── 3. Step the parafoil physics using the *previous* motor command ───
        //
        // Order matters: physics runs first (rocket was already flying with the
        // last motor command), then the controller sees the new position and
        // decides its next command.
        let (gnd_speed, vel_n, vel_e) =
            parafoil.step(motor_pos, WIND_FROM_DEG, WIND_SPEED_MS, dt);

        // ── 4. Build BLIMSDataIn from simulated state + CSV altitude ──────────
        let altitude_ft = row.altitude_m * FT_PER_M;
        let fix_type: u8 = if row.gps_valid && row.num_sats >= 4 { 3 } else { 0 };

        // Vertical velocity: use constant sink rate during descent, 0 on ascent
        let vel_d_ms = if row.altitude_m > 0.0 { SINK_RATE_MS } else { 0.0 };

        let data_in = BLIMSDataIn {
            lat:         (parafoil.lat * 1e7).round() as i32,
            lon:         (parafoil.lon * 1e7).round() as i32,
            altitude_ft,
            // Accuracy fields: use tight fixed values (simulated GPS is perfect)
            h_acc:       300,
            v_acc:       500,
            vel_n:       (vel_n   * 1000.0).round() as i32, // m/s → mm/s
            vel_e:       (vel_e   * 1000.0).round() as i32,
            vel_d:       (vel_d_ms * 1000.0).round() as i32,
            g_speed:     (gnd_speed * 1000.0).round() as i32,
            // Heading: degrees × 1e5 as i32
            head_mot:    (parafoil.heading * 1e5).round() as i32,
            s_acc:       100,
            head_acc:    50000, // 0.5 deg
            fix_type,
            gps_state:   row.gps_valid,
        };

        // ── 5. Run BLiMS guidance ─────────────────────────────────────────────
        let out_data = blims.execute(&mut hw, &data_in);

        // Save motor position so next iteration's physics uses it
        motor_pos = out_data.motor_position;

        // ── 6. Distance to target ─────────────────────────────────────────────
        let dist_m = distance_m(
            parafoil.lat, parafoil.lon,
            TARGET_LAT as f64, TARGET_LON as f64,
        );

        // ── 7. Track landing position ─────────────────────────────────────────
        // "Landed" when BLiMS enters NEUTRAL (altitude < 100 ft) — after this
        // the parafoil is in free-flight flare and we record where it is.
        if !landed {
            landing_lat = parafoil.lat;
            landing_lon = parafoil.lon;
            if altitude_ft < ALT_NEUTRAL_FT {
                landed = true;
            }
        }

        // ── 8. Write TSV row ──────────────────────────────────────────────────
        let lstr = if out_data.phase_id == 6 { loiter_step_name(out_data.loiter_step) } else { "-" };
        writeln!(out,
            "{}\t{:.2}\t{:.1}\t{:.7}\t{:.7}\t{:.1}\t{:.1}\t{:.3}\t{}\t{:.4}\t{:.5}\t{:.5}\t{:.1}",
            row.time_ms,
            row.altitude_m,
            altitude_ft,
            parafoil.lat,
            parafoil.lon,
            parafoil.heading,
            out_data.bearing,
            gnd_speed,
            phase_name(out_data.phase_id),
            out_data.motor_position,
            out_data.pid_p,
            out_data.pid_i,
            dist_m,
        ).unwrap();

        // ── 9. Log phase transitions ──────────────────────────────────────────
        if out_data.phase_id != last_phase {
            eprintln!(
                "  [t={:>10} ms | {:>7.1} ft | dist {:>6.0} m]  {} → {}",
                row.time_ms, altitude_ft, dist_m,
                phase_name(last_phase), phase_name(out_data.phase_id),
            );
            phase_entries.push((out_data.phase_id, altitude_ft));
            last_phase = out_data.phase_id;
        }
    }

    out.flush().unwrap();

    // ── Landing summary ───────────────────────────────────────────────────────
    let land_dist_m   = distance_m(landing_lat, landing_lon, TARGET_LAT as f64, TARGET_LON as f64);
    let record_dist_m = distance_m(recorded_lat, recorded_lon, TARGET_LAT as f64, TARGET_LON as f64);
    let drift_m       = distance_m(landing_lat, landing_lon, recorded_lat, recorded_lon);

    eprintln!("{}", "─".repeat(72));
    eprintln!("Simulation complete — {} iterations\n", rows.len());

    eprintln!("Phase entry altitudes:");
    for (phase_id, alt_ft) in &phase_entries {
        eprintln!("  {:<10}  {:>8.1} ft", phase_name(*phase_id), alt_ft);
    }

    eprintln!();
    eprintln!("LANDING RESULTS");
    eprintln!("  Target            : {TARGET_LAT:.6}°N  {TARGET_LON:.6}°E");
    eprintln!("  Simulated landing : {landing_lat:.6}°N  {landing_lon:.6}°E  →  {land_dist_m:>7.1} m from target");
    eprintln!("  Recorded landing  : {recorded_lat:.6}°N  {recorded_lon:.6}°E  →  {record_dist_m:>7.1} m from target");
    eprintln!("  Lateral drift vs recorded flight : {drift_m:.1} m");
}