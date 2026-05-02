# FSW 2025-2026

Written by Amira Razack (arr258) and Benjamin Zou (bwz5)

## Necessary Dependencies 
* Install rustup and Cargo (standard installation)
`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`  

* Install picotool (to flash code onto the Pico 2)  
`brew install picotool`  

* Install the correct toolchain 
`rustup target add thumbv8m.main-none-eabihf`

## Building and Running  
* Navigate into the fsw directory with `cd fsw`
* To build in release mode (for umbilical), run: `cargo build --release` 
* To flash the code onto the Pico 2, first press the BOOTSEL button on the pico and then connect it to your computer, then run: `cargo run --release`
* To build in debug mode (no connection to umbilical), run: `cargo build`
* Flash the code onto the Pico 2, press the BOOTSEL button then connect to computer: `cargo run`
* To see logs, open the /dev device that corresponds to the Pico 2 (on MacOS this is usually /dev/cu.usbmodem[random numbers]), or on Windows, download the Serial Monitor extension, access the current COM port, and click Start Monitoring.

## Next Steps
- Write integration tests for new failsafes
- Optimize QSPI flash dump speeds
