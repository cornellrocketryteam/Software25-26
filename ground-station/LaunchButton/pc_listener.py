import sys
import time
import json
import asyncio
import serial
import serial.tools.list_ports
import websockets

WEBSOCKET_URI = "ws://192.168.1.106:9000"

async def _ws_send(uri: str, command: dict, label: str):
    """Open a WebSocket, send one command, wait briefly for a response."""
    try:
        print(f"Connecting to {uri}...")
        async with websockets.connect(uri, open_timeout=5) as websocket:
            await websocket.send(json.dumps(command))
            print(f"Sent {label} to {uri} successfully!")
            try:
                response = await asyncio.wait_for(websocket.recv(), timeout=2.0)
                print(f"Received response: {response}")
            except asyncio.TimeoutError:
                print("No immediate response received (normal if server sends none).")
    except Exception as e:
        print(f"Failed to send {label}: {e}")

async def send_launch_command(uri=WEBSOCKET_URI):
    await _ws_send(uri, {"command": "launch"}, "launch command")

async def send_key_armed_command(uri=WEBSOCKET_URI):
    await _ws_send(uri, {"command": "fsw_key_arm"}, "key armed command")

async def send_key_disarmed_command(uri=WEBSOCKET_URI):
    await _ws_send(uri, {"command": "fsw_key_disarm"}, "key disarmed command")

def main():
    print("Looking for a connected Raspberry Pi Pico...")
    ports = list(serial.tools.list_ports.comports())
    pico_port = None
    
    for p in ports:
        # Firmware sets VID=0xC0DE, PID=0xCAFE, product="LaunchButton".
        # Also accept the official RPi VID (0x2E8A) in case firmware changes.
        hwid_upper = p.hwid.upper()
        if ("C0DE" in hwid_upper or "2E8A" in hwid_upper
                or "LaunchButton" in p.description
                or "Pico" in p.description
                or "Board in FS mode" in p.description):
            pico_port = p.device
            break
            
    if pico_port is None:
        print("Could not automatically find Raspberry Pi Pico.")
        print("Available serial ports:")
        for p in ports:
            print(f" - {p.device}: {p.description} [{p.hwid}]")
        
        try:
            pico_port = input("Enter the serial port to use (or Ctrl+C to exit): ").strip()
        except KeyboardInterrupt:
            print("\nExiting.")
            sys.exit(0)
            
        if not pico_port:
            sys.exit(0)

    print(f"Connecting to Pico on {pico_port} (baudrate 115200)...")
    try:
        with serial.Serial(pico_port, 115200, timeout=1) as ser:
            print("Connected. Listening for '<L>' commands from the Pico...\n")
            while True:
                if ser.in_waiting > 0:
                    try:
                        # Read and decode lines from the serial port
                        line = ser.readline().decode('utf-8').strip()
                        if line:
                            print(f"Received: {line}")
                            if '<L>' in line:
                                print("\n🚀 Launch command received from Pico!")
                                asyncio.run(send_launch_command())
                            if '<KA>' in line:
                                print("\n🚀 Key armed command received from Pico!")
                                asyncio.run(send_key_armed_command())
                            if '<KD>' in line:
                                print("\n🔒 Key disarm command received from Pico!")
                                asyncio.run(send_key_disarmed_command())
                    except UnicodeDecodeError:
                        # Ignore binary garbage on the line
                        pass
                time.sleep(0.01)
    except Exception as e:
        print(f"Serial Error: {e}")

if __name__ == '__main__':
    main()
