#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, Sender, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

// Channel from input task → USB sender task.
// Holds &'static [u8] so packet data lives in flash (string literals are 'static).
// Size 16 to absorb debug log bursts without dropping.
static CMD_CHAN: Channel<CriticalSectionRawMutex, &'static [u8], 16> = Channel::new();

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

// Mirrors the FSW usb_sender_task pattern: wait for DTR, drain the channel,
// break back to wait_connection on any endpoint error.
#[embassy_executor::task]
async fn usb_send_task(mut sender: Sender<'static, Driver<'static, USB>>) -> ! {
    loop {
        sender.wait_connection().await;
        defmt::info!("USB connected");
        loop {
            let pkt = CMD_CHAN.receive().await;
            match sender.write_packet(pkt).await {
                Ok(_) => {}
                Err(EndpointError::Disabled) => break,
                Err(_) => break,
            }
        }
        defmt::warn!("USB disconnected, waiting...");
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Cornell Rocketry");
    config.product = Some("LaunchButton");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(&mut builder, state, 64);

    let usb = builder.build();

    // Split: we only send from this side; receiver is dropped (send-only device).
    let (sender, _) = class.split();

    spawner.spawn(usb_task(usb).unwrap());
    spawner.spawn(usb_send_task(sender).unwrap());

    // Input detection loop — never touches USB directly, so it never blocks.
    let button = Input::new(p.PIN_0, Pull::Down);    // launch button
    let key    = Input::new(p.PIN_1, Pull::Down);    // key switch (arm/disarm via edges)
    let disarm = Input::new(p.PIN_3, Pull::Down);    // dedicated disarm button
    let mut prev_button = false;
    let mut prev_key    = false;
    let mut prev_disarm = false;

    // Log startup over USB so the serial monitor confirms firmware is running.
    CMD_CHAN.try_send(b"DBG:ready\n").ok();

    let mut log_tick: u32 = 0;

    loop {
        let btn      = button.is_high();
        let key_on   = key.is_high();
        let disarm_on = disarm.is_high();

        // Send a raw GPIO snapshot every 200 ticks (~2 s) over USB.
        if log_tick % 200 == 0 {
            let msg: &'static [u8] = match (btn, key_on, disarm_on) {
                (false, false, false) => b"DBG:btn=0 key=0 dis=0\n",
                (true,  false, false) => b"DBG:btn=1 key=0 dis=0\n",
                (false, true,  false) => b"DBG:btn=0 key=1 dis=0\n",
                (true,  true,  false) => b"DBG:btn=1 key=1 dis=0\n",
                (false, false, true)  => b"DBG:btn=0 key=0 dis=1\n",
                (true,  false, true)  => b"DBG:btn=1 key=0 dis=1\n",
                (false, true,  true)  => b"DBG:btn=0 key=1 dis=1\n",
                (true,  true,  true)  => b"DBG:btn=1 key=1 dis=1\n",
            };
            CMD_CHAN.try_send(msg).ok();
        }
        log_tick = log_tick.wrapping_add(1);

        // Launch button: send <L> on any state change (latching toggle).
        if btn != prev_button {
            CMD_CHAN.try_send(b"DBG:btn_edge\n").ok();
            CMD_CHAN.try_send(b"<L>\n").ok();
            Timer::after(Duration::from_millis(100)).await;
        }
        prev_button = btn;

        // Key switch: rising edge → arm, falling edge → disarm.
        if key_on && !prev_key {
            CMD_CHAN.try_send(b"DBG:key_rise\n").ok();
            CMD_CHAN.try_send(b"<KA>\n").ok();
            Timer::after(Duration::from_millis(100)).await;
        } else if !key_on && prev_key {
            CMD_CHAN.try_send(b"DBG:key_fall\n").ok();
            CMD_CHAN.try_send(b"<KD>\n").ok();
            Timer::after(Duration::from_millis(100)).await;
        }
        prev_key = key_on;

        // Dedicated disarm button (PIN_3): any state change sends <KD>.
        if disarm_on != prev_disarm {
            CMD_CHAN.try_send(b"DBG:disarm_edge\n").ok();
            CMD_CHAN.try_send(b"<X>\n").ok();
            Timer::after(Duration::from_millis(100)).await;
        }
        prev_disarm = disarm_on;

        Timer::after(Duration::from_millis(10)).await;
    }
}
