use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use src::blims::{BLIMS, Hardware};
use src::blims_state::BLIMSDataIn;

/// Landing target coordinates (decimal degrees)
const TARGET_LAT: f32 = 42.703_3;
const TARGET_LON: f32 = -77.191_2;

/// Surface wind blowing FROM this direction (deg, 0 = N, 90 = E, …)
const WIND_FROM_DEG: f32 = 200.0;

const CSV_PATH: &str = "L3_Launch4_2025.csv";

// ── Simulated hardware ────────────────────────────────────────────────────────

struct SimHardware {
    time_ms:        u32,
    pwm_level:      u16,
    enable_high:    bool,

    /// Pending alarm: (fire_at_ms, alarm_id)
    alarm:          Option<(u32, i64)>,
    next_alarm_id:  i64,
    
    /// Set by `advance_to_ms` when the alarm deadline is reached.
    /// The sim loop checks this and calls `blims.notify_loiter_alarm()`.
    pub alarm_fired: bool,
}

impl SimHardware {
    fn new() -> Self {
        Self {
            time_ms:       0,
            pwm_level:     0,
            enable_high:   false,
            alarm:         None,
            next_alarm_id: 0,
            alarm_fired:   false,
        }
    }

    /// Advance the clock and fire any pending alarm whose deadline has passed.
    fn advance_to_ms(&mut self, t: u32) {
        self.time_ms = t;
        if let Some((fire_at, _id)) = self.alarm {
            if t >= fire_at {
                self.alarm       = None;
                self.alarm_fired = true;
            }
        }
    }
}

impl Hardware for SimHardware {
    fn now_ms(&self) -> u32 {
        self.time_ms
    }

    fn set_pwm_level(&mut self, _pin: u8, level: u16) {
        self.pwm_level = level;
    }

    fn set_enable_pin(&mut self, _pin: u8, high: bool) {
        self.enable_high = high;
    }

    fn schedule_alarm_ms(&mut self, delay_ms: u32) -> i64 {
        let id      = self.next_alarm_id;
        let fire_at = self.time_ms.saturating_add(delay_ms);
        self.alarm  = Some((fire_at, id));
        self.next_alarm_id += 1;
        id
    }

    fn cancel_alarm(&mut self, alarm_id: i64) {
        if let Some((_at, id)) = self.alarm {
            if id == alarm_id {
                self.alarm = None;
            }
        }
        self.alarm_fired = false;
    }
}

// ── CSV row ───────────────────────────────────────────────────────────────────

/// One row from L3_Launch4_2025.csv, already converted to BLIMSDataIn units.
struct CsvRow {
    /// Original timestamp in milliseconds (for clock injection)
    time_ms:     u32,
    /// Altitude in metres (kept for TSV output)
    altitude_m:  f32,
    /// Altitude in feet AGL (passed to BLiMS)
    altitude_ft: f32,
    /// Latitude  × 1e7  (integer degrees, matches UBX-NAV-PVT)
    lat:         i32,
    /// Longitude × 1e7
    lon:         i32,
    h_acc:       u32,
    v_acc:       u32,
    vel_n:       i32, // mm/s
    vel_e:       i32,
    vel_d:       i32,
    g_speed:     i32, // mm/s
    /// Heading of motion × 1e5  (degrees, already in this form in the CSV)
    head_mot:    i32,
    s_acc:       u32,
    /// Heading accuracy estimate × 1e5 (already in this form in the CSV)
    head_acc:    u32,
    /// 3 = 3-D fix, 0 = no fix
    fix_type:    u8,
    gps_state:   bool,
}

const FT_PER_M: f32 = 3.2808;

// ── Column-index finder ───────────────────────────────────────────────────────

/// Returns the column index for a header name, or panics with a clear message.
fn col(header: &[&str], name: &str) -> usize {
    header
        .iter()
        .position(|h| h.trim() == name)
        .unwrap_or_else(|| panic!("CSV missing column: {name}"))
}

// ── CSV loader ────────────────────────────────────────────────────────────────

fn load_csv(path: &str) -> Vec<CsvRow> {
    let file   = File::open(path).unwrap_or_else(|e| panic!("Cannot open {path}: {e}"));
    let reader = BufReader::new(file);
    let mut lines  = reader.lines();

    // ── Parse header ─────────────────────────────────────────────────────────
    let header_line = lines
        .next()
        .expect("CSV is empty")
        .expect("IO error reading header");
    let header: Vec<&str> = header_line.split(',').collect();

    let ci_ts   = col(&header, "Timestamp");
    let ci_alt  = col(&header, "Altitude");
    let ci_gps  = col(&header, "GPS_Status");
    let ci_sats = col(&header, "Number_of_Satellites");
    let ci_lon  = col(&header, "Longitude");
    let ci_lat  = col(&header, "Latitude");
    let ci_hacc = col(&header, "Horizontal_Accuracy");
    let ci_vacc = col(&header, "Vertical_Accuracy");
    let ci_vn   = col(&header, "NED_North_Velocity");
    let ci_ve   = col(&header, "NED_East_Velocity");
    let ci_vd   = col(&header, "NED_Down_Velocity");
    let ci_gs   = col(&header, "Ground_Speed");
    let ci_hm   = col(&header, "Heading_of_Motion");
    let ci_sa   = col(&header, "Speed_Accuracy_Estimate");
    let ci_ha   = col(&header, "Heading_Accuracy_Estimate");

    // ── Parse data rows ───────────────────────────────────────────────────────
    let mut rows = Vec::new();

    for line_result in lines {
        let line = match line_result {
            Ok(l)  => l,
            Err(_) => continue,
        };

        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 20 { continue; }

        let get_f32 = |i: usize| -> f32 { f.get(i).unwrap_or(&"").trim().parse().unwrap_or(0.0) };
        let get_f64 = |i: usize| -> f64 { f.get(i).unwrap_or(&"").trim().parse().unwrap_or(0.0) };
        let get_i32 = |i: usize| -> i32 { f.get(i).unwrap_or(&"").trim().parse().unwrap_or(0)   };
        let get_u8  = |i: usize| -> u8  { f.get(i).unwrap_or(&"").trim().parse().unwrap_or(0)   };

        // NED velocities in CSV are m/s – convert to mm/s for BLIMSDataIn
        let mms = |i: usize| -> i32 { (get_f32(i) * 1000.0).round() as i32 };

        let gps_ok   = get_u8(ci_gps) == 1;
        let num_sats = get_u8(ci_sats);
        let fix_type: u8 = if gps_ok && num_sats >= 4 { 3 } else { 0 };

        let altitude_m  = get_f32(ci_alt);
        let altitude_ft = altitude_m * FT_PER_M;

        // Timestamp: CSV column is seconds → convert to ms
        let time_ms = (get_f64(ci_ts) * 1000.0).round() as u32;

        // head_mot / head_acc are already in deg×1e5 in the CSV
        let head_mot = get_f64(ci_hm).round() as i32;
        let head_acc = get_f64(ci_ha).round() as u32;

        rows.push(CsvRow {
            time_ms,
            altitude_m,
            altitude_ft,
            lat:       get_i32(ci_lat),
            lon:       get_i32(ci_lon),
            h_acc:     get_f32(ci_hacc).round() as u32,
            v_acc:     get_f32(ci_vacc).round() as u32,
            vel_n:     mms(ci_vn),
            vel_e:     mms(ci_ve),
            vel_d:     mms(ci_vd),
            g_speed:   mms(ci_gs),
            head_mot,
            s_acc:     get_f32(ci_sa).round() as u32,
            head_acc,
            fix_type,
            gps_state: gps_ok,
        });
    }

    rows
}

// ── Phase/LoiterStep name helpers ─────────────────────────────────────────────

fn phase_name(id: i8) -> &'static str {
    match id {
        0 => "HELD",
        1 => "TRACK",
        2 => "DOWNWIND",
        3 => "BASE",
        4 => "FINAL",
        5 => "NEUTRAL",
        6 => "LOITER",
        _ => "?",
    }
}

fn loiter_step_name(id: i8) -> &'static str {
    match id {
        0 => "TURN_RIGHT",
        1 => "PAUSE_RIGHT",
        2 => "TURN_LEFT",
        3 => "PAUSE_LEFT",
        _ => "-",
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    // ── Load flight data ──────────────────────────────────────────────────────
    eprintln!("Loading {}…", CSV_PATH);
    let rows = load_csv(CSV_PATH);
    eprintln!("Loaded {} rows.", rows.len());

    // ── Initialise controller and hardware mock ───────────────────────────────
    let mut hw    = SimHardware::new();
    let mut blims = BLIMS::new();

    blims.begin(&mut hw, /*pwm_pin=*/ 0, /*enable_pin=*/ 1);
    blims.set_target(TARGET_LAT, TARGET_LON);
    blims.set_wind_from_deg(WIND_FROM_DEG);

    eprintln!("Target    : {TARGET_LAT:.6}°N  {TARGET_LON:.6}°E");
    eprintln!("Wind FROM : {WIND_FROM_DEG:.1}°");
    eprintln!("{}", "─".repeat(72));

    // ── TSV output ────────────────────────────────────────────────────────────
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    writeln!(
        out,
        "Time_ms\tAlt_m\tAlt_ft\tLat\tLon\tGndSpd_ms\t\
         Heading_deg\tBearing_deg\tPhase\tMotorPos\t\
         PID_P\tPID_I\tLoiterStep"
    )
    .unwrap();

    // ── Simulation loop ───────────────────────────────────────────────────────
    let mut last_phase: i8 = -1;

    // Collect phase-entry altitudes for the summary table
    let mut phase_entry: [(i8, f32); 16] = [(-1, 0.0); 16];
    let mut phase_entry_count = 0usize;

    for row in &rows {
        // 1. Advance the simulated clock; this fires loiter alarms when due.
        hw.advance_to_ms(row.time_ms);

        // 2. Propagate fired alarm to the controller.
        if hw.alarm_fired {
            hw.alarm_fired = false;
            blims.notify_loiter_alarm();
        }

        // 3. Build the input packet.
        let data_in = BLIMSDataIn {
            lon:         row.lon,
            lat:         row.lat,
            altitude_ft: row.altitude_ft,
            h_acc:       row.h_acc,
            v_acc:       row.v_acc,
            vel_n:       row.vel_n,
            vel_e:       row.vel_e,
            vel_d:       row.vel_d,
            g_speed:     row.g_speed,
            head_mot:    row.head_mot,
            s_acc:       row.s_acc,
            head_acc:    row.head_acc,
            fix_type:    row.fix_type,
            gps_state:   row.gps_state,
        };

        // 4. Run one guidance step.
        let out_data = blims.execute(&mut hw, &data_in);

        // 5. Emit a TSV row.
        let heading_deg = row.head_mot as f64 * 1e-5;
        let lstr = if out_data.phase_id == 6 {
            loiter_step_name(out_data.loiter_step)
        } else {
            "-"
        };

        writeln!(
            out,
            "{}\t{:.2}\t{:.1}\t{:.7}\t{:.7}\t{:.3}\t{:.1}\t{:.1}\t{}\t{:.4}\t{:.5}\t{:.5}\t{}",
            row.time_ms,
            row.altitude_m,
            row.altitude_ft,
            row.lat  as f64 * 1e-7,
            row.lon  as f64 * 1e-7,
            row.g_speed as f32 / 1000.0,
            heading_deg,
            out_data.bearing,
            phase_name(out_data.phase_id),
            out_data.motor_position,
            out_data.pid_p,
            out_data.pid_i,
            lstr,
        )
        .unwrap();

        // 6. Log phase transitions to stderr.
        if out_data.phase_id != last_phase {
            eprintln!(
                "  [t={:>10} ms | {:>8.1} ft]  {} → {}",
                row.time_ms,
                row.altitude_ft,
                phase_name(last_phase),
                phase_name(out_data.phase_id),
            );
            if phase_entry_count < phase_entry.len() {
                phase_entry[phase_entry_count] = (out_data.phase_id, row.altitude_ft);
                phase_entry_count += 1;
            }
            last_phase = out_data.phase_id;
        }
    }

    out.flush().unwrap();

    // ── Summary ───────────────────────────────────────────────────────────────
    eprintln!("{}", "─".repeat(72));
    eprintln!("Simulation complete – {} iterations.", rows.len());
    eprintln!("\nPhase entry altitudes:");
    for (phase_id, alt_ft) in &phase_entry[..phase_entry_count] {
        eprintln!("  {:<10}  {:>8.1} ft", phase_name(*phase_id), alt_ft);
    }
}
