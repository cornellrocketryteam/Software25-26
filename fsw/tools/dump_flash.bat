@echo off
REM dump_flash.bat — Cornell Rocketry Team
REM Dumps onboard QSPI flash CSV over USB umbilical and saves to a .csv file.
REM
REM Usage (from anywhere):
REM   tools\dump_flash.bat            <- auto-detect COM port
REM   tools\dump_flash.bat COM4       <- explicit port
REM   tools\dump_flash.bat COM4 115200
REM
REM NOTE: close any serial monitor on the same port before running.

cd /d "%~dp0dump_flash"
cargo run --release -- %*
