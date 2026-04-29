#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_rp::usb::{Driver, InterruptHandler}; // Added USB drivers
use embassy_rp::peripherals::USB;                // Added USB peripheral
use embassy_rp::bind_interrupts;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::UsbDevice;
use embassy_time::{Duration, Timer};
use fixed::FixedU16;
use fixed::types::extra::U4;
use defmt_rtt as _;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const TOP: u16 = 65535; //setting to max value for u16
const DIVIDER: f32 = 45.78; 

fn set_motor_position(pwm: &mut Pwm<'_>, config: &mut PwmConfig, position: f32) {
    let five_percent_duty = TOP as f32 * 0.05;
    let duty = (five_percent_duty + position * five_percent_duty) as u16;

    config.compare_a = duty;
    pwm.set_config(config);
}

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    // This initializes the 'log' crate and pipes it through USB Serial
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger_task(driver).expect("logger task failed to spawn"));
    Timer::after(Duration::from_secs(10)).await;
    log::info!("USB Serial initialized! Motor test starting...");

    let mut state_pin = Output::new(p.PIN_0, Level::High);

    let mut pwm_config = PwmConfig::default();
    pwm_config.top = TOP;
    pwm_config.divider = FixedU16::<U4>::from_num(DIVIDER);
    pwm_config.compare_a = 0;
    pwm_config.enable = true;

    let mut pwm = Pwm::new_output_a(p.PWM_SLICE6, p.PIN_28, pwm_config.clone());

    set_motor_position(&mut pwm, &mut pwm_config, 0.5);
    log::info!("[main] Moving to neutral (0.5)");
    Timer::after(Duration::from_secs(10)).await; 

    state_pin.set_low(); 
    log::info!("pulse low");
    Timer::after(Duration::from_millis(500)).await; 

    
    state_pin.set_high(); 
    log::info!("enable");
    Timer::after(Duration::from_secs(5)).await;

    log::info!("entering test loop");

    loop {
        
        log::info!("[main] Moving to +10.5 turns (1.0)");
        // log::info!("[main] Moving to +10.5 turns (0.75)");
        set_motor_position(&mut pwm, &mut pwm_config, 1.0);
        Timer::after(Duration::from_secs(10)).await;
 
        log::info!("[main] Moving to neutral 0 turns (0.5)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.5);
        Timer::after(Duration::from_secs(10)).await;
 
        log::info!("[main] Moving to -10.5 turns (0.0)");
        // log::info!::info!("[main] Moving to +10.5 turns (0.25)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.0);
        Timer::after(Duration::from_secs(10)).await;

        log::info!("[main] Moving to -10.5 turns (0.5)");
        set_motor_position(&mut pwm, &mut pwm_config, 0.5);
        Timer::after(Duration::from_secs(10)).await;
}

    // loop {
    //     log::info!("hello");
    // }
}


