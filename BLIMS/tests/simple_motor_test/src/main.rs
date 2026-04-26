use pico::hal::{
    clocks::ClocksManager,
    gpio::{FunctionPwm, Pin, Pins, ValidPinMode},
    pwm::{Channel, FreeRunning, Pwm, Slice},
    sio::Sio,
    watchdog::Watchdog,
    pac,
};
use cortex_m_rt::entry;
use embedded_time::rate::*;
use panic_halt as _;
use core::fmt::Write;
use core::fmt::Write as FmtWrite;

const PWM_PIN_NUM: u8 = 28;
const WRAP_CYCLE_COUNT: u16 = 65535;

fn setup_pwm_50hz(pwm: &mut Slice<FreeRunning>, pin: &mut Pin<FunctionPwm>) {
    // The RP2040 clock frequency is 125 MHz by default
    let clock_freq: u32 = 125_000_000;
    let pwm_freq: u32 = 50;

    // Calculate divider integer and fractional parts
    let divider_int = clock_freq / pwm_freq / WRAP_CYCLE_COUNT as u32;
    let remainder = clock_freq % (pwm_freq * WRAP_CYCLE_COUNT as u32);
    let divider_frac = (remainder * 16) / (pwm_freq * WRAP_CYCLE_COUNT as u32);

    pwm.set_clkdiv_int_frac(divider_int as u8, divider_frac as u8);
    pwm.set_wrap(WRAP_CYCLE_COUNT);
    pwm.enable();
}

fn set_motor_position(pwm: &mut Slice<FreeRunning>, channel: Channel, position: f32) {
    // Clamp position between 0.0 and 1.0
    let pos = position.clamp(0.0, 1.0);

    // Map position to duty cycle between 5% and 10%
    let five_percent_duty_cycle = (WRAP_CYCLE_COUNT as f32 * 0.05) as u16;
    let duty = five_percent_duty_cycle + (pos * five_percent_duty_cycle as f32) as u16;

    pwm.set_channel_level(channel, duty);
}

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let clocks = ClocksManager::new(pac.CLOCKS);

    let sio = Sio::new(pac.SIO);

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Initialize PWM pin 28 as PWM function
    let mut pwm_pin = pins.gpio28.into_mode::<FunctionPwm>();

    // Get PWM slice and channel for pin 28
    let pwm_slices = Pwm::new(pac.PWM, &mut pac.RESETS);
    let mut pwm_slice = pwm_slices.slice(pwm_pin.id().num);
    let pwm_channel = pwm_pin.id().channel;

    // Configure PWM slice for free running mode
    pwm_slice.set_ph_correct();
    pwm_slice.enable();

    setup_pwm_50hz(&mut pwm_slice, &mut pwm_pin);

    // Setup UART for printing (using cortex_m_semihosting or similar)
    // For simplicity, use defmt or panic_halt for output in embedded environment
    // Here, we just simulate prints with semihosting or RTT if available

    loop {
        // Simulate printf with cortex_m_semihosting or RTT or other logging
        // Here, just a placeholder comment
        // println!("start sequence");

        set_motor_position(&mut pwm_slice, pwm_channel, 0.0);
        cortex_m::asm::delay(3_000_000); // approx 3000 ms delay at 1 MHz (adjust as needed)

        // println!("finish turn 1");

        set_motor_position(&mut pwm_slice, pwm_channel, 0.5);
        cortex_m::asm::delay(3_000_000);

        set_motor_position(&mut pwm_slice, pwm_channel, 1.0);
        cortex_m::asm::delay(3_000_000);

        // println!("finish sequence");
    }
}