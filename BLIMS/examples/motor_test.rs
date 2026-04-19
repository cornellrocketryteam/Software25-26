#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_time::{Duration, Timer};
use fixed::FixedU16;
use fixed::types::extra::U4;
use {defmt_rtt as _, panic_probe as _};

const TOP: u16 = 65535; //setting to max value for u16
const DIVIDER: f32 = 45.78; 

fn set_motor_position(pwm: &mut Pwm<'_>, config: &mut PwmConfig, position: f32) {
    let five_percent_duty = TOP as f32 * 0.05;
    let duty = (five_percent_duty + position * five_percent_duty) as u16;

    config.compare_a = duty;
    pwm.set_config(config);
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut state_pin = Output::new(p.PIN_0, Level::High);

    let mut pwm_config = PwmConfig::default();
    pwm_config.top = TOP;
    pwm_config.divider = FixedU16::<U4>::from_num(DIVIDER);
    pwm_config.compare_a = 0;
    pwm_config.enable = true;

    let mut pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone());

    set_motor_position(&mut pwm, &mut pwm_config, 0.5);
    Timer::after(Duration::from_secs(5)).await; 

    state_pin.set_low(); 
    defmt::println!("pulse low");
    Timer::after(Duration::from_millis(500)).await; 

    
    // state_pin.set_high(); 
    // defmt::println!("enable");
    // Timer::after(Duration::from_secs(5)).await;


    loop {
        
        defmt::println!("[main] Moving to +10.5 turns (0.75)");
        // log::info!("[main] Moving to +10.5 turns (0.75)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.75);
        Timer::after(Duration::from_secs(5)).await;
 
        defmt::println!("[main] Moving to neutral 0 turns (0.5)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.5);
        Timer::after(Duration::from_secs(5)).await;
 
        defmt::println!("[main] Moving to -10.5 turns (0.25)");
        // log::info!("[main] Moving to +10.5 turns (0.25)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.25);
        Timer::after(Duration::from_secs(5)).await;

        defmt::println!("[main] Moving to -10.5 turns (0.25)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.5);
        Timer::after(Duration::from_secs(5)).await;
}

    // loop {
    //     defmt::println!("hello");
    // }
}


