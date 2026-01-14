# FSW 2025-2026

Written by Benjamin Zou (bwz5)

See Project Updates [here](https://github.com/orgs/cornellrocketryteam/projects/4/views/1)

## Necessary Dependencies 
* Install rustup and Cargo
`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`  

* Install picotool (to flash code onto the Pico 2)  
`brew install picotool`  

* Install the correct toolchain 
`rustup target add thumbv7em-none-eabihf`
`rustup target add thumbv8m.main-none-eabihf`

## Building and Running  
* To build in release mode, run: `cargo build --release` 
* To flash the code onto the Pico 2, run: `cargo run --release`  

## TODO: 
- Update the FlightState initializer function 
- transition and execute in FlightState should behave accurately 
- Implement umbilical 
- Implement actuators 
- Implement other sensors
- Implement SPI Flash storage 



