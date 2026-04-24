"""
serial_splitter.py — BLiMS serial port splitter (Mac)
======================================================
Reads the USB CDC-ACM output from the Pico (car_test.rs) and mirrors it onto
two virtual PTY ports so that both car_test_visualizer.py AND a raw terminal
(e.g. 'screen' or 'minicom') can read the same stream simultaneously.

Usage
-----
1. Flash car_test.rs onto the Pico and plug it in via USB.
2. Run:
       python serial_splitter.py
   The script will auto-detect the Pico's /dev/cu.usbmodem* port, or you can
   pass the port explicitly:
       python serial_splitter.py /dev/cu.usbmodem1101

3. The script prints two virtual port paths like:
       Virtual port A: /dev/ttys005   ← pass to car_test_visualizer.py
       Virtual port B: /dev/ttys006   ← open in a terminal with 'screen /dev/ttys006'

4. Launch the visualizer in a second terminal:
       python car_test_visualizer.py /dev/ttys005

Press Ctrl-C to stop.
"""

import glob
import os
import pty
import sys
import threading
import time

import serial

# ── Port auto-detection ───────────────────────────────────────────────────────

def find_pico_port() -> str:
    """Return the first /dev/cu.usbmodem* device found, or raise."""
    candidates = glob.glob('/dev/cu.usbmodem*')
    if not candidates:
        raise RuntimeError(
            "No /dev/cu.usbmodem* device found.\n"
            "Check that the Pico is plugged in and car_test.rs is flashed."
        )
    if len(candidates) > 1:
        print(f"[splitter] Multiple usbmodem ports found: {candidates}")
        print(f"[splitter] Using {candidates[0]} — pass a port argument to override.")
    return candidates[0]

# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    # Determine physical serial port
    if len(sys.argv) > 1:
        phys_port = sys.argv[1]
    else:
        phys_port = find_pico_port()

    print(f"[splitter] Opening {phys_port} @ 115200 baud")

    try:
        ser = serial.Serial(phys_port, 115200, timeout=1)
    except serial.SerialException as e:
        print(f"[splitter] ERROR: {e}")
        sys.exit(1)

    # Give the CDC-ACM device a moment to settle after open
    time.sleep(0.5)

    # Create two virtual PTY pairs
    master_a, slave_a = pty.openpty()
    master_b, slave_b = pty.openpty()

    port_a = os.ttyname(slave_a)
    port_b = os.ttyname(slave_b)

    print()
    print(f"  Virtual port A (→ visualizer):  {port_a}")
    print(f"  Virtual port B (→ terminal):     {port_b}")
    print()
    print(f"  Run the visualizer with:")
    print(f"      python car_test_visualizer.py {port_a}")
    print()
    print(f"  Monitor raw output with:")
    print(f"      screen {port_b} 115200")
    print()
    print("[splitter] Forwarding — press Ctrl-C to stop.")

    stop_event = threading.Event()

    def read_and_forward():
        while not stop_event.is_set():
            try:
                waiting = ser.in_waiting
                data = ser.read(waiting if waiting > 0 else 1)
            except serial.SerialException as e:
                print(f"\n[splitter] Serial read error: {e}")
                stop_event.set()
                break

            if data:
                try:
                    os.write(master_a, data)
                except OSError:
                    pass  # visualizer not open yet — ignore EPIPE
                try:
                    os.write(master_b, data)
                except OSError:
                    pass

    t = threading.Thread(target=read_and_forward, daemon=True)
    t.start()

    try:
        while not stop_event.is_set():
            time.sleep(0.1)
    except KeyboardInterrupt:
        print("\n[splitter] Stopped.")
    finally:
        stop_event.set()
        ser.close()
        os.close(master_a)
        os.close(master_b)

if __name__ == '__main__':
    main()