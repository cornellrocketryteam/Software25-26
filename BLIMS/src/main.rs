#![no_std]
#![no_main]
 
mod blims_constants;
mod blims_state;
mod blims;
 
use blims::Blims;
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
 
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
 
    // Enable pin (PIN_0) – BLiMS::new() will drive it high
    let enable_pin = Output::new(p.PIN_0, Level::Low);
 
    // PWM – PIN_28, PWM_SLICE6 channel A
    let mut pwm_config = PwmConfig::default();
    pwm_config.top      = TOP;
    pwm_config.divider  = FixedU16::<U4>::from_num(DIVIDER);
    pwm_config.compare_a = 0;
    pwm_config.enable   = true;
 
    let pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone());
 
    // Construct BLiMS (drives enable high and parks motor at neutral)
    let mut blims = Blims::new(pwm, pwm_config, enable_pin);
 
    // ── Pre-flight configuration ─────────────────────────────────────────────
    //TODO replace lon, lat with. actual target
    blims.set_target(123.0, 456.0);   
    blims.set_wind_from_deg(270.0);    
 
 
    defmt::println!("BLiMS initialised – waiting for parafoil stabilisation");
    Timer::after(Duration::from_secs(5)).await;
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
 