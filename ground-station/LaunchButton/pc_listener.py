import sys
import time
import json
import asyncio
import serial
import serial.tools.list_ports
import websockets

WEBSOCKET_URI = "ws://127.0.0.1:9000"

async def send_launch_command(uri=WEBSOCKET_URI):
    try:
        print(f"Connecting to {uri}...")
        async with websockets.connect(uri) as websocket:
            # The JSON payload matching the fill-station's Command enum
            command = {"command": "launch"}
            await websocket.send(json.dumps(command))
            print(f"Sent launch command to {uri} successfully!")
            
            # Optionally wait for a brief moment to receive a response
            try:
                response = await asyncio.wait_for(websocket.recv(), timeout=2.0)
                print(f"Received response: {response}")
            except asyncio.TimeoutError:
                print("No immediate response received (this is normal if no response is expected).")
    except Exception as e:
        print(f"Failed to send launch command: {e}")

def main():
    print("Looking for a connected Raspberry Pi Pico...")
    ports = list(serial.tools.list_ports.comports())
    pico_port = None
    
    for p in ports:
        # Check standard MicroPython Pico descriptors or hardware IDs
        # 2E8A:0005 is the typical VID:PID for Raspberry Pi Pico with MicroPython
        if "2E8A" in p.hwid or "Pico" in p.description or "Board in FS mode" in p.description:
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
                    except UnicodeDecodeError:
                        # Ignore binary garbage on the line
                        pass
                time.sleep(0.01)
    except Exception as e:
        print(f"Serial Error: {e}")

if __name__ == '__main__':
    main()
