#!/usr/bin/env python3
"""
Umbilical Test Tool — mimics how the fill station communicates with the FSW.

Reads text lines from USB serial:
  - Lines starting with "$TELEM," are parsed as CSV telemetry
  - All other lines are printed as FSW logs

Commands can be typed interactively:
  L  = Launch        M  = Open MAV      m  = Close MAV
  S  = Open SV       s  = Close SV      V  = Safe
  D  = Reset Card    F  = Reset FRAM    R  = Reboot
  G  = Dump Flash    W  = Wipe Flash    I  = Flash Info
  1-4 = Payload N1-N4

Usage:
  python3 umbilical_test.py                        # auto-detect port
  python3 umbilical_test.py /dev/cu.usbmodem1234   # specify port
"""

import sys
import glob
import threading
import serial

BAUD = 115200

TELEM_FIELDS = [
    "flight_mode", "pressure", "temp", "altitude",
    "latitude", "longitude", "num_satellites", "timestamp",
    "mag_x", "mag_y", "mag_z",
    "accel_x", "accel_y", "accel_z",
    "gyro_x", "gyro_y", "gyro_z",
    "pt3", "pt4", "rtd",
    "sv_open", "mav_open",
]

COMMANDS = {
    "L": "Launch",
    "M": "Open MAV",
    "m": "Close MAV",
    "S": "Open SV",
    "s": "Close SV",
    "V": "Safe",
    "D": "Reset Card",
    "F": "Reset FRAM",
    "R": "Reboot",
    "G": "Dump Flash",
    "W": "Wipe Flash",
    "I": "Flash Info",
    "1": "Payload N1",
    "2": "Payload N2",
    "3": "Payload N3",
    "4": "Payload N4",
}


def find_port():
    """Auto-detect the Pico CDC-ACM port."""
    patterns = [
        "/dev/cu.usbmodem*",    # macOS
        "/dev/ttyACM*",         # Linux
    ]
    for pat in patterns:
        ports = glob.glob(pat)
        if ports:
            return ports[0]
    return None


def parse_telemetry(csv_str):
    """Parse a $TELEM CSV line into a dict."""
    parts = csv_str.split(",")
    if len(parts) < len(TELEM_FIELDS):
        return None
    result = {}
    for i, name in enumerate(TELEM_FIELDS):
        val = parts[i].strip()
        if name in ("flight_mode", "num_satellites", "sv_open", "mav_open"):
            result[name] = int(val)
        else:
            result[name] = float(val)
    return result


MODE_NAMES = {
    0: "Startup", 1: "Standby", 2: "Ascent", 3: "Coast",
    4: "DrogueDeployed", 5: "MainDeployed", 6: "Fault",
}


def reader_thread(ser):
    """Continuously read lines from serial and parse them."""
    while True:
        try:
            raw = ser.readline()
            if not raw:
                continue
            line = raw.decode("utf-8", errors="replace").strip()
            if not line:
                continue

            if line.startswith("$TELEM,"):
                telem = parse_telemetry(line[7:])  # skip "$TELEM,"
                if telem:
                    mode = MODE_NAMES.get(telem["flight_mode"], "Unknown")
                    print(
                        f"\033[36m[TELEM]\033[0m mode={mode}  "
                        f"alt={telem['altitude']:.1f}m  "
                        f"pres={telem['pressure']:.0f}Pa  "
                        f"temp={telem['temp']:.1f}C  "
                        f"lat={telem['latitude']:.4f}  "
                        f"lon={telem['longitude']:.4f}  "
                        f"sats={telem['num_satellites']}  "
                        f"sv={'OPEN' if telem['sv_open'] else 'closed'}  "
                        f"mav={'OPEN' if telem['mav_open'] else 'closed'}"
                    )
                else:
                    print(f"\033[33m[TELEM PARSE ERROR]\033[0m {line}")
            else:
                print(f"\033[90m[FSW] {line}\033[0m")

        except serial.SerialException:
            print("\033[31m[DISCONNECTED]\033[0m")
            break
        except Exception as e:
            print(f"\033[31m[ERROR]\033[0m {e}")


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else find_port()
    if not port:
        print("No serial port found. Usage: python3 umbilical_test.py /dev/cu.usbmodemXXXX")
        sys.exit(1)

    print(f"Connecting to {port} at {BAUD} baud...")
    ser = serial.Serial(port, BAUD, timeout=1)
    print(f"Connected. Reading telemetry...\n")

    # Print command help
    print("Commands (type letter + Enter):")
    for key, desc in COMMANDS.items():
        print(f"  {key}  = {desc}")
    print()

    # Start background reader
    t = threading.Thread(target=reader_thread, args=(ser,), daemon=True)
    t.start()

    # Interactive command loop
    try:
        while True:
            user = input()
            cmd_char = user.strip()
            if cmd_char in COMMANDS:
                token = f"<{cmd_char}>"
                ser.write(token.encode())
                print(f"\033[32m[SENT]\033[0m {token} ({COMMANDS[cmd_char]})")
            elif cmd_char.lower() == "q":
                break
            elif cmd_char:
                print(f"Unknown command '{cmd_char}'. Valid: {', '.join(COMMANDS.keys())}")
    except (KeyboardInterrupt, EOFError):
        pass

    print("\nClosing.")
    ser.close()


if __name__ == "__main__":
    main()
