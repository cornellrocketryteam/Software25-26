from machine import UART, Pin
import time

# --- 1. Motor Signal Setup ---
# Group all the pins (LED, Azimuth, Elevation)
signal_pin_numbers = [28, 6, 7, 8, 9, 10, 11]
test_pins = [Pin(pin, Pin.OUT) for pin in signal_pin_numbers]

# --- 2. GPS Setup ---
# Initialize UART0 for the GPS at 9600 baud
gps_uart = UART(0, baudrate=9600, tx=Pin(0), rx=Pin(1), timeout=100)

print("Starting Combined GPS & Motor Signal Test...")
print("Waiting for GPS NMEA sentences...")

# Timer variables
last_toggle_time = time.ticks_ms()
toggle_state = 0

while True:
    # --- Check for GPS Data ---
    if gps_uart.any():
        raw_data = gps_uart.readline()
        
        if raw_data is not None:
            try:
                if isinstance(raw_data, bytes):
                    line = raw_data.decode('utf-8').strip()
                else:
                    line = raw_data.strip()

                # 3. Only print if it's not an empty line
                if line: 
                    print("GPS:", line)
            except Exception:
                # Ignore occasional garbled startup bytes
                pass
            
    # --- Toggle Motor Signals (Every 2000 ms) ---
    current_time = time.ticks_ms()
    if time.ticks_diff(current_time, last_toggle_time) >= 2000:
        # Flip the state
        toggle_state = not toggle_state
        
        # Apply to all pins
        for p in test_pins:
            p.value(toggle_state)
            
        # Print status to the console
        state_str = "HIGH (5V at buffers)" if toggle_state else "LOW (0V at buffers)"
        print(f"\n---> Motor Signals Toggled: {state_str} <--- \n")
        
        # Reset the timer
        last_toggle_time = current_time
        
    # Tiny delay to keep the microcontroller from running at 100% CPU
    time.sleep(0.01)