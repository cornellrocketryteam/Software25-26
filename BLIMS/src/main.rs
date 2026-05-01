#![no_std]
#![no_main]
 
mod blims_constants;
mod blims_state;
mod blims;
 
use blims::{Blims, Hardware};
use blims_state::BlimsDataIn;
 
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Timer};
use fixed::FixedU16;
use fixed::types::extra::U4;
use {defmt_rtt as _, panic_probe as _};
 

const TOP: u16    = 65_535;
const DIVIDER: f32 = 38.15;
 
// Control loop period
const LOOP_PERIOD_MS: u64 = 50; // 20 Hz

// ── Wind profile — update pre-flight from HRRR/radiosonde data ───────────────
const WIND_PROFILE_SIZE: usize = 11;
const WIND_ALTITUDES_M: [f32; WIND_PROFILE_SIZE] = [
    0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 500.0, 550.0, 610.0,
];
const WIND_DIRS_DEG: [f32; WIND_PROFILE_SIZE] = [
    270.0, 270.0, 270.0, 270.0, 270.0, 270.0, 270.0, 270.0, 270.0, 270.0, 270.0,
];
 
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
 
    // Enable pin (PIN_0) – BLiMS::new() will drive it high
    let enable_pin = Output::new(p.PIN_0, Level::Low); // pin 34 for av bay, pin 0 for breadboard
 
    // PWM – PIN_28, PWM_SLICE6 channel A
    let mut pwm_config = PwmConfig::default();
    pwm_config.top      = TOP;
    pwm_config.divider  = FixedU16::<U4>::from_num(DIVIDER);
    pwm_config.compare_a = 0; // compare_b for av bay, compare_a for breadboard – see comments in motor_test.rs
    pwm_config.enable   = true;
 
    let pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone()); //compare_b and slice 9 pin 35 for av bay, compare_a and slice 6 pin 28 for breadboard
 
    // Construct BLiMS (drives enable high and parks motor at neutral)
    let mut blims = Blims::new(Hardware { pwm, pwm_config, enable_pin });
 
    // ── Pre-flight configuration ─────────────────────────────────────────────
    //TODO replace lon, lat with. actual target
    blims.set_upwind_target(42.705565, -77.196310);
    blims.set_downwind_target(42.703311, -77.181125);
    blims.set_wind_from_deg(270.0);   
    blims.set_wind_profile(&WIND_ALTITUDES_M, &WIND_DIRS_DEG); 
 
 
    defmt::println!("BLiMS initialised – waiting for parafoil stabilisation");
    defmt::println!(" entering control loop");
 
    // ── Control loop ─────────────────────────────────────────────────────────
    loop {
        // TODO: replace this stub with real data from your GPS / barometer task.
        // In the full FSW this struct is populated via a shared-memory channel
        // or mutex from the sensor task.
        
        let data_in = BlimsDataIn {
            lat:         424_441_000,   // 42.4441° × 1e7
            lon:        -764_821_000,   // −76.4821° × 1e7
            altitude_ft: 1200.0,        // feet AGL from barometer
            vel_n:        0,
            vel_e:        0,
            vel_d:        500,           // 0.5 m/s descent
            g_speed:      5_000,         // 5 m/s ground speed
            head_mot:     0,             // heading of motion (degrees × 1e5)
            fix_type:     3,             // 3-D fix
            gps_state:   true,
            h_acc:        2_000,         // 2 m horizontal accuracy
            v_acc:        3_000,
            s_acc:        500,
            head_acc:     100_000,       // 1.0° × 1e5
        };
 
        blims.execute(&data_in);
 
        
 
        Timer::after(Duration::from_millis(LOOP_PERIOD_MS)).await;
    }
}
 