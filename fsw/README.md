# FSW 2025-2026

Written by Benjamin Zou (bwz5)

See Project Updates [here](https://github.com/orgs/cornellrocketryteam/projects/4/views/1)

## Necessary Dependencies 
* Install rustup and Cargo (standard installation)
`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`  

* Install picotool (to flash code onto the Pico 2)  
`brew install picotool`  

* Install the correct toolchain 
`rustup target add thumbv8m.main-none-eabihf`

## Building and Running  
* Navigate into the fsw directory with `cd fsw`
* To build in release mode, run: `cargo build --release` 
* To flash the code onto the Pico 2, first press the BOOTSEL button on the pico and then connect it to your computer, then run: `cargo run --release`
* To see logs, open the /dev device that corresponds to the Pico 2 (on MacOS this is usually /dev/cu.usbmodem[random numbers]

## TODO: 
- transition and execute in FlightState should behave accurately 
- Implement umbilical commanding 
- Implement actuators 
- Correct packet structure 
- Testing for faults 
- Storing flight data



