from machine import UART, Pin
import time

LED_PIN = 26 
led = Pin(LED_PIN, Pin.OUT)

# Initialize UART0 for the RFD900x at 115200 baud
radio_uart = UART(0, baudrate=115200, tx=Pin(0), rx=Pin(1), timeout=100)

print("Starting RFD900x Local Connection & LED Test...")

# Turn LED ON to indicate the test sequence is starting
led.value(1)

# The Guard Time (1.5 seconds of absolute silence)
print("Waiting for guard time...")
time.sleep(1.5)

# Send the escape sequence
print("Sending '+++' escape sequence...")
radio_uart.write(b'+++')

# Wait for the radio to process and reply
time.sleep(1.5)

# Read the response
if radio_uart.any():
    response = radio_uart.read()
    
    if response is not None:
        print(f"\nRadio Replied: {response}")
        
        if b'OK' in response:
            print(">>> SUCCESS! Two-way UART connection is working. <<<")
            
            # Ask the radio for its firmware version
            print("\nAsking for firmware version...")
            radio_uart.write(b'ATI\r')
            time.sleep(0.5)
            
            # Repeat the pattern: Read, check for None, then process
            info = radio_uart.read()
            if info is not None:
                print(f"Firmware Info: {info}")
                
            # Exit command mode
            radio_uart.write(b'ATO\r')
    else:
        print("\nRead timed out: No data received.")
else:
    print("\nNo response from radio. Check TX/RX wiring and power!")

# Step 5: Fall into an infinite LED heartbeat loop
print("\nTest complete. Entering continuous LED heartbeat loop...")
while True:
    led.toggle()
    time.sleep(1)