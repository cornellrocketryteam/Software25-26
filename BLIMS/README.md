# BLIMS (Steerable Parachute Payload)

BLIMS (Balloon-Launched Intelligent Mechanism System) is the steerable parachute payload for the rocket. It operates its own flight software and controls steering lines to navigate to a target coordinate.

## Building and Running
* To build in release mode, run: `cargo build`
* To build car_test.rs, run:  `cargo build --example blims_car_test --release`
* To flash the code onto the Pico 2, run: `cargo run --release` 
* To run on car_test.rs: `cargo run --example blims_car_test --release`