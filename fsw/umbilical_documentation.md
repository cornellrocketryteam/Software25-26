# Umbilical Implementation Guide

This document describes the "umbilical" communication feature implemented in commit `416ea471` (on the `ml/umbilical` branch). It serves as both an analysis of what changed in that commit and a guide on how to implement this exact sending/receiving mechanism on a completely different `embassy-rp` Rust codebase that doesn't currently have any of this code.

## 1. Overview of the Commit `416ea471` Changes

To fully grasp the umbilical feature, here is a summary of all the files modified in commit `416ea471`:

1. **`fsw/Cargo.toml`**: Added the `embassy-usb` and `log` crates as dependencies.
2. **`fsw/src/module.rs`**: Added a new function, `init_usb_device`, to configure the USB driver as a CDC-ACM device (Virtual Serial Port) with custom USB Vendor/Product IDs (`0xc0de` / `0xcafe`). It also updated the `embassy-rp` module imports to include USB peripherals.
3. **`fsw/src/main.rs`**: Introduced the three new asynchronous tasks (`usb_task`, `umbilical_sender_task`, and `umbilical_receiver_task`) to handle background USB communications.
4. **`nix/dev-shells/default.nix`**: Added `thumbv8m.main-none-eabihf` to the Rust toolchain target list to ensure the codebase can compile for the microcontroller architecture.

---

## 2. Implementing the Umbilical on a New Codebase

To add this capability to a new project, you'll replicate the core structure of the umbilical over USB CDC-ACM. The magic of this approach is that it makes the embedded microcontroller appear simply as a standard Serial/COM port to the connected host computer.

### A. Important Caveats on the Current State
Before copying this implementation directly, please be aware of the following caveats regarding its current form:

* **Separation of Concerns:** The umbilical routines run as separate background `embassy` tasks, completely independent from the main flight loop. Regardless of the current flight mode, whether the main loop is blocked, or what the rocket is doing, it will **always** be able to receive a command and send data through the umbilical tasks.
* **Smart Connection Handshake:** When the USB cable is **not connected**, the sender and receiver tasks smartly suspend (`wait_connection().await`). They will do exactly zero work until a physical connection is established.
* **USB Resource Conflicts (Logs vs. Umbilical):** The RP2040/RP2350 microcontroller only has *one* USB peripheral. Therefore, **you cannot access standard USB logs and run the umbilical at the same time**. 
* **Currently Sends Dummy Data:** The implementation does not send meaningful data yet; it merely transmits a placeholder byte array (`[0, 1, 2, ..., 29]`).
* **Serialization/Deserialization Needed:** Because both the flight computer and the ground station host computer are written in Rust, a future improvement is highly recommended: instead of manually packing bytes, use a `#[derive(Serialize, Deserialize)]` struct (using a `no_std` crate like `postcard` or `bincode`) to pass data back and forth easily.

### B. The Setup and Conditional Compilation

To handle the "Logs vs. Umbilical" conflict, you should use conditional compilation. In your `main.rs`, spawn the umbilical tasks when compiling in `release` mode, but keep the USB open for standard logging when compiling in `debug` mode.

```rust
// Inside main() after initializing the USB driver:

#[cfg(debug_assertions)]
{
    // In Debug mode, use the USB for standard logging
    spawner.spawn(logger_task(usb_driver)).unwrap();
}

#[cfg(not(debug_assertions))]
{
    // In Release mode, use the USB for the umbilical
    let (usb_device, mut class) = module::init_usb_device(usb_driver);
    let (sender, receiver) = class.split();
    
    spawner.spawn(usb_task(usb_device)).unwrap();
    spawner.spawn(umbilical_sender_task(sender)).unwrap();
    spawner.spawn(umbilical_receiver_task(receiver)).unwrap();
}
```

### C. The Core Async Tasks

The functionality hinges on three asynchronous tasks. Here are the exact lines of code for each:

#### 1. The USB Background Task
This keeps the USB peripheral alive and processes low-level protocol details.
```rust
#[embassy_executor::task]
async fn usb_task(mut usb_device: UsbDevice<'static, module::UsbDriver>) -> ! {
    usb_device.run().await
}
```

#### 2. The Umbilical Sender Task
This task streams telemetry data to the connected computer. The current exact lines are:
```rust
#[embassy_executor::task]
async fn umbilical_sender_task(mut sender: Sender<'static, module::UsbDriver>) -> ! {
    loop {
        sender.wait_connection().await;

        loop {
            let mut buf = [0; 30];
            for i in 0..buf.len() {
                buf[i] = i as u8;
            }

            match sender.write_packet(&buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow shouldn't be possible"),
                Err(EndpointError::Disabled) => break,
            };

            // Only write an outgoing umbilical packet every 100 ms
            Timer::after_millis(100).await;
        }
    }
}
```
**Timing Modification:** The sending interval takes **100 milliseconds** per loop (10 sends per second). To modify the amount of time it waits between sends (the sending rate), change the `Timer::after_millis(100).await;` line to your desired duration.

#### 3. The Umbilical Receiver Task
This task receives incoming byte commands from the computer and acts strictly as a command parser. The current exact lines are:
```rust
#[embassy_executor::task]
async fn umbilical_receiver_task(mut receiver: Receiver<'static, module::UsbDriver>) -> ! {
    let mut buf = [0; 64];
    loop {
        receiver.wait_connection().await;

        loop {
            let n = match receiver.read_packet(&mut buf).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => panic!("Buffer overflow isn't possible"),
                Err(EndpointError::Disabled) => break,
            };

            let data = &buf[..n];

            match data {
                b"<L>" => {}, // Launch
                b"<M>" => {}, // Open MAV
                b"<m>" => {}, // Close MAV
                b"<S>" => {}, // Open SV
                b"<s>" => {}, // Close SV

                b"<V>" => {}, // Safe

                b"<D>" => {}, // Reset card
                b"<F>" => {}, // Reset fram

                b"<R>" => {}, // Reboot

                b"<C1>" => {}, // Change target lat
                b"<C2>" => {}, // Change target long
                b"<C3>" => {}, // Change ref pressure
                b"<C4>" => {}, // Change alt state
                b"<C5>" => {}, // Change card state
                b"<C6>" => {}, // Change alt armed
                b"<C7>" => {}, // Change flight mode

                _ => {}, // Unknown command
            }

            // Only listen for an incoming umbilical packet every 100 ms
            Timer::after_millis(100).await;
        }
    }
}
```

**Commands Supported:** The umbilical currently accepts the following 3-byte standard command string tokens:
* `<L>` : Launch
* `<M>` : Open MAV (Mechanically Actuated Valve)
* `<m>` : Close MAV
* `<S>` : Open SV (Solenoid Valve)
* `<s>` : Close SV
* `<V>` : Safe vehicle
* `<D>` : Reset SD card
* `<F>` : Reset FRAM
* `<R>` : Reboot
* `<C1>` to `<C7>` : Override standard flight states (change target lat/long, ref pressure, alt state, card state, alt armed, flight mode).

**Timing Modification:** The receiving interval checks for new commands every **100 milliseconds**. To modify the polling time, alter the `Timer::after_millis(100).await;` line at the bottom of the receiver loop.
