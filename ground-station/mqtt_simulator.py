import paho.mqtt.client as mqtt
import json
import time
import random
import math

# ==============================================================================
# CONFIGURATION
# ==============================================================================
MQTT_BROKER_IP = "192.168.8.193" 
MQTT_PORT = 1883
MQTT_TOPIC = "rats/raw/1"  # unit_id 0 = Fill Station
PUBLISH_RATE_HZ = 10       # Telemetry publish rate in Hz

# ==============================================================================
# MQTT SETUP
# ==============================================================================
def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print(f"Connected to MQTT Broker at {MQTT_BROKER_IP}")
    else:
        print(f"Failed to connect, return code {rc}")

client = mqtt.Client()
client.on_connect = on_connect

print(f"Connecting to {MQTT_BROKER_IP}...")
client.connect(MQTT_BROKER_IP, MQTT_PORT, 60)
client.loop_start()

# ==============================================================================
# SIMULATION LOOP
# ==============================================================================
print("Starting telemetry simulation... Press Ctrl+C to stop.")

# Base values for simulation
fill_progress = 0.0
time_step = 0.0

try:
    while True:
        # Simulate gradual pressurization and filling
        time_step += 0.1
        fill_progress = (fill_progress + 0.5) % 150.0  # Loops 0 to 50kg
        
        # Build the JSON payload matching the Rust Packet + Fill Station schema
        payload = {
            # Top-Level Radio
            "sync_word": 4277009100,
            
            # Shared Telemetry (Rust 'Packet')
            "flight_mode": 1,
            "pressure": 101.3 + random.uniform(-0.1, 0.1),
            "temp": 22.5 + random.uniform(-0.2, 0.2),
            "altitude": 140.0 + random.uniform(-1, 1),
            
            "latitude": 42.4440,
            "longitude": -76.4832,
            "num_satellites": 12,
            "timestamp": time.time(),
            
            "mag_x": random.uniform(-50, 50),
            "mag_y": random.uniform(-50, 50),
            "mag_z": random.uniform(-50, 50),
            
            "accel_x": random.uniform(-0.1, 0.1),
            "accel_y": random.uniform(-0.1, 0.1),
            "accel_z": 9.81 + random.uniform(-0.1, 0.1),
            
            "gyro_x": random.uniform(-1, 1),
            "gyro_y": random.uniform(-1, 1),
            "gyro_z": random.uniform(-1, 1),
            
            "pt3": 100.0 + (math.sin(time_step) * 20) + random.uniform(-2, 2),
            "pt4": 50.0 + fill_progress * 2 + random.uniform(-1, 1),          
            "rtd": 20.0 + random.uniform(-0.5, 0.5),                       
            
            "sv_open": bool(int(time_step) % 10 < 5),                          
            "mav_open": False,
            
            # Fill Station Specific
            "pt_1_pressure": 4500.0 + random.uniform(-10, 10),                
            "pt_2_pressure": 800.0 + random.uniform(-5, 5),                   
            "ball_valve_open": True,                                           
            "sv_1_open": True,                                                 
            "sv_2_open": False,                                                
            "load_cell": fill_progress + random.uniform(-0.1, 0.1),            
            "ignition": False,                                                 
            "qd_state": int((time_step // 2) % 5)                              
        }
        
        # Publish to EMQX
        client.publish(MQTT_TOPIC, json.dumps(payload))
        print(f"Published to {MQTT_TOPIC} | Load Cell: {payload['load_cell']:.1f}kg | PT4: {payload['pt4']:.1f}psi")
        
        # Wait to match target publish rate (10Hz)
        time.sleep(1.0 / PUBLISH_RATE_HZ)

except KeyboardInterrupt:
    print("\nSimulation stopped by user.")
    client.loop_stop()
    client.disconnect()
