import machine
import os
import time
import vfs

# Attempt to import the SD card driver
try:
    import sdcard
except ImportError:
    print("ERROR: 'sdcard.py' not found on the RP2350!")
    print("Please upload sdcard.py to the board before running this test.")
    raise

# ==========================================
# HARDWARE SETUP
# ==========================================
SPI_ID = 1
SCK_PIN = 10
MOSI_PIN = 11
MISO_PIN = 12
CS_PIN = 13

print("Initializing SPI bus...")
# Initialize SPI. Start with a slow baudrate (1MHz) for the initialization phase.
# The sdcard library will automatically ramp the speed up once it connects.
spi = machine.SPI(SPI_ID, 
                  baudrate=1000000, 
                  polarity=0, 
                  phase=0, 
                  sck=machine.Pin(SCK_PIN), 
                  mosi=machine.Pin(MOSI_PIN), 
                  miso=machine.Pin(MISO_PIN))

# Initialize the Chip Select (CS) pin
cs = machine.Pin(CS_PIN, machine.Pin.OUT)

# ==========================================
# THE TEST ROUTINE
# ==========================================
try:
    print("Attempting to talk to the SD Card...")
    sd = sdcard.SDCard(spi, cs)
    
    print("SD Card detected! Mounting file system...")
    
    # Create a Virtual File System (VFS) and mount it to the directory '/sd'
    vfs_obj = vfs.VfsFat(sd)
    vfs.mount(vfs_obj, "/sd")
    print("Mounted successfully at '/sd'")
    
    # 1. WRITE TEST
    test_file = "/sd/telemetry_test.txt"
    print(f"\nWriting test data to {test_file}...")
    with open(test_file, "w") as f:
        f.write("RP2350 SD Card Interface is ACTIVE!\n")
        f.write(f"Timestamp: {time.ticks_ms()}\n")
        
    # 2. READ TEST
    print("Reading data back...")
    with open(test_file, "r") as f:
        data = f.read()
        print("--- FILE CONTENTS ---")
        print(data.strip())
        print("---------------------")
        
    # 3. DIRECTORY LISTING
    print("\nListing all files on SD card:")
    files = os.listdir("/sd")
    for file in files:
        print(f" - {file}")

    # CLEANUP
    vfs.umount("/sd")
    print("\nSD Card safely unmounted. Test PASSED!")

except OSError as e:
    print("\n--- TEST FAILED ---")
    print("Hardware or filesystem error detected.")
    print(f"Error code: {e}")
    print("\nTroubleshooting checklist:")
    print("1. Are the MISO/MOSI/SCK/CS pins correct for your board?")
    print("2. Is the microSD card formatted to FAT32?")
    print("3. Is the microSD card physically pushed all the way into the slot?")
except Exception as e:
    print(f"\nUnexpected Error: {e}")