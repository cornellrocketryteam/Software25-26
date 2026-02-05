import asyncio
import websockets
import json
import random
import time

# ============================================================================
# CONSTANTS & CONFIGURATION (Matched to src/main.rs)
# ============================================================================

# scaling for a PT with range to 1500
PT1500_SCALE = 0.909754
PT1500_OFFSET = 5.08926

# scaling for a PT with range to 2000
PT2000_SCALE = 1.22124
PT2000_OFFSET = 5.37052

# scaling for a LoadCell
LOADCELL_SCALE = 1.69661
LOADCELL_OFFSET = 75.37882

ADC_SAMPLE_RATE_HZ = 10
SAFETY_TIMEOUT_SECONDS = 15

# ============================================================================
# STATE
# ============================================================================

state = {
    "sv": {
        "SV1": {"actuated": False, "continuity": True},
        "SV2": {"actuated": False, "continuity": True},
        "SV3": {"actuated": False, "continuity": True},
        "SV4": {"actuated": False, "continuity": True},
        "SV5": {"actuated": False, "continuity": True},
    },
    "mav": {
        "angle": 0.0, 
        "pulse_width_us": 1000
    },
    "ball_valve": {
        "signal": False, # Low
        "on_off": False  # Low
    },
    "igniters": {
        1: {"continuity": True},
        2: {"continuity": True},
    },
    "adc_stream_active": False
}

# Helper to reset safety state (simulating emergency shutdown)
def perform_emergency_shutdown():
    print("!!! EMERGENCY SHUTDOWN TRIGGERED !!!")
    # Close all SVs
    for sv in state["sv"]:
        state["sv"][sv]["actuated"] = False
    
    # Close MAV
    state["mav"]["angle"] = 0.0
    state["mav"]["pulse_width_us"] = 1000

    # Ball Valve is not explicitly mentioned in emergency shutdown in main.rs, 
    # but logically usually stays as is or goes safe. The Rust code mainly closes SVs and MAV.
    # We will stick to what main.rs does.

# ============================================================================
# WEB SOCKET HANDLER
# ============================================================================

async def handler(websocket):
    client_addr = websocket.remote_address
    print(f"Client connected: {client_addr}")
    
    # Connection Lifecycle State
    last_heartbeat = time.time()
    adc_task = None
    
    try:
        while True:
            # We need to check for timeout + receive messages concurrently
            # Python's asyncio.wait_for can timeout the receive, helping us check the heartbeat periodically
            
            try:
                # Wait for a message with a short timeout to check heartbeat frequently
                # or rely on the read timeout to wake us up to check logic
                message = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                
                # received a message, reset heartbeat
                last_heartbeat = time.time()
                
                # Update safety state: if we recovered from a timeout (though usually we disconnect),
                # but here we just reset the timer.
                
                await process_message(websocket, message)
                
            except asyncio.TimeoutError:
                # No message received in 1s. Check if total silence > 15s
                if time.time() - last_heartbeat > SAFETY_TIMEOUT_SECONDS:
                    print(f"Client {client_addr} timed out (no heartbeat for {SAFETY_TIMEOUT_SECONDS}s). Disconnecting.")
                    perform_emergency_shutdown()
                    await websocket.close()
                    break
                else:
                    # Just a tick, continue loop
                    continue
            
            # Check ADC stream state management
            if state["adc_stream_active"]:
                 if adc_task is None or adc_task.done():
                     adc_task = asyncio.create_task(stream_adc(websocket))
            elif not state["adc_stream_active"]:
                 if adc_task and not adc_task.done():
                     adc_task.cancel()
                     try:
                         await adc_task
                     except asyncio.CancelledError:
                         pass
                     adc_task = None

    except websockets.exceptions.ConnectionClosed:
        print(f"Client {client_addr} disconnected")
    except Exception as e:
        print(f"Unexpected error: {e}")
    finally:
        state["adc_stream_active"] = False
        if adc_task and not adc_task.done():
            adc_task.cancel()

async def process_message(websocket, message_text):
    try:
        data = json.loads(message_text)
        command = data.get("command")
        print(f"RX: {data}")
        
        response = {"type": "success"} # Default response

        if command == "heartbeat":
            pass # Just keeps connection alive

        # --- Solenoid Valves ---
        elif command == "get_valve_state":
            valve = data.get("valve")
            if valve in state["sv"]:
                response = {
                    "type": "valve_state",
                    "actuated": state["sv"][valve]["actuated"],
                    "continuity": state["sv"][valve]["continuity"]
                }
            else:
                response = {"type": "error", "message": f"Unknown valve: {valve}"}

        elif command == "actuate_valve":
            valve = data.get("valve")
            val = data.get("state") # bool
            if valve in state["sv"]:
                state["sv"][valve]["actuated"] = val
            else:
                response = {"type": "error", "message": f"Unknown valve: {valve}"}

        # --- MAV ---
        elif command == "get_mav_state":
            response = {
                "type": "mav_state",
                "angle": state["mav"]["angle"],
                "pulse_width_us": state["mav"]["pulse_width_us"]
            }
        
        elif command == "set_mav_angle":
            angle = float(data.get("angle", 0.0))
            state["mav"]["angle"] = angle
            # Approx conversion: 0deg=1000us, 90deg=2000us -> us = 1000 + (angle/90)*1000
            # This is a rough mock approximation
            state["mav"]["pulse_width_us"] = int(1000 + (angle / 90.0) * 1000)
            
        elif command == "mav_open":
            state["mav"]["angle"] = 90.0
            state["mav"]["pulse_width_us"] = 2000

        elif command == "mav_close":
            state["mav"]["angle"] = 0.0
            state["mav"]["pulse_width_us"] = 1000
            
        elif command == "mav_neutral":
             state["mav"]["pulse_width_us"] = 1300
             state["mav"]["angle"] = 27.0 # Approx

        # --- Igniters ---
        elif command == "get_igniter_continuity":
            ign_id = data.get("id")
            if ign_id in state["igniters"]:
                response = {
                    "type": "igniter_continuity",
                    "id": ign_id,
                    "continuity": state["igniters"][ign_id]["continuity"]
                }
            else:
                response = {"type": "error", "message": "Unknown igniter ID"}

        elif command == "ignite":
            # Simulate non-blocking ignition
            asyncio.create_task(simulate_ignition())

        # --- ADC Streaming ---
        elif command == "start_adc_stream":
            state["adc_stream_active"] = True

        elif command == "stop_adc_stream":
            state["adc_stream_active"] = False
        
        # --- Ball Valve ---
        elif command == "bv_open":
            asyncio.create_task(simulate_bv_sequence("open"))
            
        elif command == "bv_close":
            asyncio.create_task(simulate_bv_sequence("close"))
            
        elif command == "bv_signal":
            # "high", "low", "open", "close", "true", "false"
            val_str = str(data.get("state")).lower()
            if val_str in ["high", "open", "true", "on"]:
                state["ball_valve"]["signal"] = True
            elif val_str in ["low", "close", "false", "off"]:
                state["ball_valve"]["signal"] = False
            else:
                response = {"type": "error", "message": "Invalid signal state"}

        elif command == "bv_on_off":
             val_str = str(data.get("state")).lower()
             if val_str in ["high", "open", "true", "on"]:
                 state["ball_valve"]["on_off"] = True
             elif val_str in ["low", "close", "false", "off"]:
                 state["ball_valve"]["on_off"] = False
             else:
                 response = {"type": "error", "message": "Invalid ON/OFF state"}

        else:
            response = {"type": "error", "message": f"Unknown command: {command}"}

        await websocket.send(json.dumps(response))

    except Exception as e:
        print(f"Error processing message: {e}")
        err_response = {"type": "error", "message": str(e)}
        try:
            await websocket.send(json.dumps(err_response))
        except:
            pass

# ============================================================================
# SIMULATION TASKS
# ============================================================================

async def simulate_ignition():
    print("Ignition sequence started (mock)")
    await asyncio.sleep(3.0)
    print("Ignition sequence completed (mock)")

async def simulate_bv_sequence(mode):
    # Match the timing of the real sequence roughly
    # Open: Signal HIGH + ON_OFF HIGH -> 3s -> ON_OFF LOW
    # Close: Signal LOW + ON_OFF HIGH -> 3s -> ON_OFF LOW
    print(f"Ball Valve {mode} sequence started")
    
    if mode == "open":
        state["ball_valve"]["signal"] = True
    else:
        state["ball_valve"]["signal"] = False
        
    state["ball_valve"]["on_off"] = True
    await asyncio.sleep(3.0) # Acutation time
    state["ball_valve"]["on_off"] = False
    print(f"Ball Valve {mode} sequence completed")

async def stream_adc(websocket):
    try:
        while state["adc_stream_active"]:
            # Generate fake data
            # ADC1 Ch0: PT1500
            # ADC1 Ch1-3: PT2000
            # ADC2 Ch0, 2, 3: PT2000
            # ADC2 Ch1: LOADCELL
            
            adc1_ch0_raw = random.randint(100, 3000)
            adc1_ch1_raw = random.randint(100, 3000)
            
            adc2_ch1_raw = random.randint(1000, 2000) # Load cell

            msg = {
                "type": "adc_data",
                "timestamp_ms": int(time.time() * 1000),
                "valid": True,
                "adc1": [
                    # Ch0: PT1500
                    scale_reading(adc1_ch0_raw, PT1500_SCALE, PT1500_OFFSET),
                    # Ch1: PT2000
                    scale_reading(adc1_ch1_raw, PT2000_SCALE, PT2000_OFFSET),
                    # Ch2: PT2000 (Empty)
                    scale_reading(0, PT2000_SCALE, PT2000_OFFSET),
                    # Ch3: PT2000 (Empty)
                    scale_reading(0, PT2000_SCALE, PT2000_OFFSET),
                ],
                "adc2": [
                    # Ch0: PT2000
                    scale_reading(random.randint(0, 100), PT2000_SCALE, PT2000_OFFSET), 
                    # Ch1: LOADCELL
                    scale_reading(adc2_ch1_raw, LOADCELL_SCALE, LOADCELL_OFFSET),
                    # Ch2: PT2000
                    scale_reading(0, PT2000_SCALE, PT2000_OFFSET),
                    # Ch3: PT2000
                    scale_reading(0, PT2000_SCALE, PT2000_OFFSET),
                ]
            }
            try:
                await websocket.send(json.dumps(msg))
            except websockets.exceptions.ConnectionClosed:
                break
                
            await asyncio.sleep(1.0 / ADC_SAMPLE_RATE_HZ)
            
    except asyncio.CancelledError:
        pass
    except Exception as e:
        print(f"Stream error: {e}")

def scale_reading(raw, scale, offset):
    if raw == 0:
         return {"raw": 0, "voltage": 0.0, "scaled": None}
    
    # 4.096V range, 16-bit (roughly, though ADS1015 is 12-bit, code uses raw 0-2047 often or signed)
    # The rust code treats raw as i16 but ADS1015 in default config is 12-bit.
    # We will just simulate 'raw' values and 'voltage' loosely.
    voltage = (raw / 2047.0) * 4.096 
    scaled_val = (raw * scale) + offset
    return {
        "raw": raw,
        "voltage": round(voltage, 3),
        "scaled": round(scaled_val, 3)
    }

async def main():
    async with websockets.serve(handler, "0.0.0.0", 9000):
        print("Mock Server started on ws://0.0.0.0:9000")
        await asyncio.Future()  # run forever

if __name__ == "__main__":
    asyncio.run(main())
