# Prerequisite: pip install paho-mqtt
import paho.mqtt.client as mqtt
import json
import time
import random
import math

# ==============================================================================
# CONFIGURATION
# ==============================================================================
MQTT_BROKER_IP = "192.168.1.206" 
MQTT_PORT = 1883
MQTT_TOPIC = "rats/raw/1"  # unit_id 0 = Fill Station, 1 = RATS
PUBLISH_RATE_HZ = 20       # Telemetry publish rate in Hz

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
        # Simulate gradual pressurization, filling, and movement
        time_step += 0.1
        fill_progress = (fill_progress + 0.5) % 150.0  # Loops 0 to 50kg
        
        # Calculate seconds since midnight UTC (simulating MAX-M10S Time of Day)
        gps_time_of_day = time.time() % 86400
        
        # Simulate the microcontrollers' internal hardware clocks (milliseconds since boot)
        # Using time_step ensures it is perfectly monotonic for your velocity math!
        sys_time_ms = int(time_step * 1000)
        
        # Build the JSON payload matching the unified Schema
        payload = {
            # Top-Level Radio
            "sync_word": 4277009100,
            
            # Shared Telemetry (Rust 'Packet')
            "flight_mode": 1,
            "pressure": 101.3 + random.uniform(-0.1, 0.1),
            "temp": 22.5 + random.uniform(-0.2, 0.2),
            "altitude": 140.0 + (time_step * 15.0) + random.uniform(-1, 1), # Simulating ascent for velocity math!
            
            "latitude": 42.4440,
            "longitude": -76.4832,
            "num_satellites": 12,
            "timestamp": gps_time_of_day,
            
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
            
            "sv_2_open": bool(int(time_step) % 10 < 5),                          
            "mav_open": False,
            
            "ms_since_boot_cfc": sys_time_ms,
            "ms_since_boot_fill": sys_time_ms,

            # Event Flags
            "ssa_drogue_deployed": 0,
            "ssa_main_deployed": 0,
            "cmd_n1": 0, "cmd_n2": 0, "cmd_n3": 0, "cmd_n4": 0,
            "cmd_a1": 0, "cmd_a2": 0, "cmd_a3": 0,

            # Airbrake & Control States
            "airbrake_state": 0,
            "predicted_apogee": 10500.0 + random.uniform(-50, 50),

            # Advanced GPS / U-Blox Metrics
            "h_acc": 1500 + int(random.uniform(-100, 100)),        
            "v_acc": 2000 + int(random.uniform(-100, 100)),        
            "vel_n": 25.0 + random.uniform(-1, 1),                 
            "vel_e": 5.0 + random.uniform(-0.5, 0.5),              
            "vel_d": -150.0 + random.uniform(-5, 5),               
            "g_speed": 25.49 + random.uniform(-1, 1),              
            "s_acc": 150,                                          
            "head_acc": 250000,                                    
            "fix_type": 3,                                         
            "head_mot": 1131000 + int(random.uniform(-5000, 5000)),

            # BLiMS Outputs
            "blims_motor_position": math.sin(time_step) * 5.0,
            "blims_phase_id": 2,
            "blims_pid_p": 1.5,
            "blims_pid_i": 0.05,
            "blims_bearing": 11.3 + random.uniform(-0.1, 0.1),
            "blims_loiter_step": 0,
            "blims_heading_des": 11.0,
            "blims_heading_error": 0.3,
            "blims_error_integral": 0.02,
            "blims_dist_to_target_m": 4500.0 - (time_step * 25.0),

            # BLiMS Config
            "blims_target_lat": 42.4500,
            "blims_target_lon": -76.4800,
            "blims_wind_from_deg": 270.0,
            
            # Fill Station Specific
            "pt_1_pressure": 4500.0 + random.uniform(-10, 10),                
            "pt_2_pressure": 800.0 + random.uniform(-5, 5),                   
            "ball_valve_open": True,                                           
            "sv_1_open": True,                                                 
            "load_cell": fill_progress + random.uniform(-0.1, 0.1),            
            "ignition": False,                                                 
            "qd_state": int((time_step // 2) % 5)                              
        }
        
        # Publish to EMQX
        client.publish(MQTT_TOPIC, json.dumps(payload))
        print(f"Published to {MQTT_TOPIC} | Sys Time: {sys_time_ms}ms | GPS Time: {gps_time_of_day:.3f}s")
        
        # Wait to match target publish rate (10Hz)
        time.sleep(1.0 / PUBLISH_RATE_HZ)

except KeyboardInterrupt:
    print("\nSimulation stopped by user.")
    client.loop_stop()
    client.disconnect()