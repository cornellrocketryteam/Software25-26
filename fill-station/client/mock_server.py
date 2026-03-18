import asyncio
import websockets
import json
import random
import time

# Mock State
state = {
    "sv": {
        "SV1": {"actuated": False, "continuity": True},
        "SV2": {"actuated": False, "continuity": True},
        "SV3": {"actuated": False, "continuity": True},
        "SV4": {"actuated": False, "continuity": True},
        "SV5": {"actuated": False, "continuity": True},
    },
    "mav": {"angle": 0.0, "pulse_width_us": 1000},
    "bv": {"signal": "low", "on_off": "low"},
    "qd": {"steps": 0, "state": "closed"},
    "igniters": {
        1: {"continuity": True},
        2: {"continuity": True},
    },
    "adc_stream_active": False,
    "fsw_stream_active": False,
    "fsw": {
        "flight_mode": 1,
        "pressure": 101325.0,
        "temp": 25.0,
        "altitude": 0.0,
        "latitude": 42.0,
        "longitude": -71.0,
        "num_satellites": 8,
        "timestamp": 0.0,
        "mag_x": 0.0, "mag_y": 0.0, "mag_z": 0.0,
        "accel_x": 0.0, "accel_y": 0.0, "accel_z": 9.81,
        "gyro_x": 0.0, "gyro_y": 0.0, "gyro_z": 0.0,
        "pt3": 0.0,
        "pt4": 0.0,
        "rtd": 25.0,
        "sv_open": False,
        "mav_open": False,
    }
}

async def handler(websocket):
    print(f"Client connected: {websocket.remote_address}")
    try:
        # Start background tasks for streaming if active
        adc_task = None
        fsw_task = None
        
        async for message in websocket:
            data = json.loads(message)
            command = data.get("command")
            print(f"Received: {data}")
            
            response = {"type": "success"}

            if command == "get_valve_state":
                valve = data.get("valve")
                if valve in state["sv"]:
                    response = {
                        "type": "valve_state",
                        "valve": valve,
                        "actuated": state["sv"][valve]["actuated"],
                        "continuity": state["sv"][valve]["continuity"]
                    }
                else:
                    response = {"type": "error", "message": "Unknown valve"}

            elif command == "actuate_valve":
                valve = data.get("valve")
                val = data.get("state") # true/false
                if valve in state["sv"]:
                    state["sv"][valve]["actuated"] = val
                else:
                    response = {"type": "error", "message": "Unknown valve"}

            elif command == "get_mav_state":
                response = {
                    "type": "mav_state",
                    "angle": state["mav"]["angle"],
                    "pulse_width_us": state["mav"]["pulse_width_us"]
                }
            
            elif command == "set_mav_angle":
                angle = data.get("angle")
                state["mav"]["angle"] = angle
                # rough conversion for mock
                state["mav"]["pulse_width_us"] = int(1000 + (angle / 90.0) * 1000)
            
            elif command == "mav_open":
                state["mav"]["angle"] = 90.0
                state["mav"]["pulse_width_us"] = 2000

            elif command == "mav_close":
                state["mav"]["angle"] = 0.0
                state["mav"]["pulse_width_us"] = 1000
            
            elif command == "mav_neutral":
                 state["mav"]["pulse_width_us"] = 1300
                 # angle approx 27? doesn't matter for mock
                 state["mav"]["angle"] = 27.0

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
                pass

            elif command == "start_adc_stream":
                state["adc_stream_active"] = True
                if adc_task is None or adc_task.done():
                    adc_task = asyncio.create_task(stream_adc(websocket))

            elif command == "stop_adc_stream":
                state["adc_stream_active"] = False
                if adc_task:
                    adc_task.cancel()
            
            # Ball Valve
            elif command == "bv_open":
                state["bv"]["signal"] = "high"
            elif command == "bv_close":
                state["bv"]["signal"] = "low"
            elif command == "bv_signal":
                state["bv"]["signal"] = data.get("state", "low")
            elif command == "bv_on_off":
                state["bv"]["on_off"] = data.get("state", "low")
            
            # QD Commands
            elif command == "qd_move":
                direction = data.get("direction", True)
                steps = data.get("steps", 0)
                if direction:
                    state["qd"]["steps"] += steps
                else:
                    state["qd"]["steps"] -= steps
            elif command == "qd_retract":
                state["qd"]["state"] = "retracted"
                state["qd"]["steps"] = 1000
            elif command == "qd_extend":
                state["qd"]["state"] = "extended"
                state["qd"]["steps"] = 0

            # FSW Commands
            elif command in ["fsw_launch", "fsw_open_mav", "fsw_close_mav", "fsw_open_sv", "fsw_close_sv", "fsw_safe", "fsw_reset_fram", "fsw_reset_card", "fsw_reboot", "fsw_dump_flash", "fsw_wipe_flash", "fsw_flash_info", "fsw_payload_n1", "fsw_payload_n2", "fsw_payload_n3", "fsw_payload_n4"]:
                # Simply succeed, we could update fake fsw state if we wanted
                pass

            elif command == "start_fsw_stream":
                state["fsw_stream_active"] = True
                if fsw_task is None or fsw_task.done():
                    fsw_task = asyncio.create_task(stream_fsw(websocket))
            
            elif command == "stop_fsw_stream":
                state["fsw_stream_active"] = False
                if fsw_task:
                    fsw_task.cancel()
            
            elif command == "heartbeat":
                pass

            await websocket.send(json.dumps(response))

    except websockets.exceptions.ConnectionClosed:
        print("Client disconnected")
    except Exception as e:
        print(f"Error: {e}")
    finally:
        state["adc_stream_active"] = False
        state["fsw_stream_active"] = False


async def stream_adc(websocket):
    start_time = time.time()
    try:
        while state["adc_stream_active"]:
            msg = {
                "type": "adc_data",
                "timestamp_ms": int(time.time() * 1000),
                "valid": True,
                "adc1": [
                    {"raw": random.randint(1000, 1050), "voltage": 2.5, "scaled": 500.0 + random.uniform(-1, 1)},
                    {"raw": random.randint(0, 50), "voltage": 0.1, "scaled": 10.0},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                ],
                "adc2": [
                    {"raw": random.randint(2000, 2047), "voltage": 3.3, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                ]
            }
            await websocket.send(json.dumps(msg))
            await asyncio.sleep(0.01) # 100 Hz
    except asyncio.CancelledError:
        pass
    except Exception as e:
        print(f"ADC Stream error: {e}")

async def stream_fsw(websocket):
    try:
        while state["fsw_stream_active"]:
            fsw_tel = state["fsw"].copy()
            fsw_tel["timestamp"] += 0.1
            state["fsw"]["timestamp"] = fsw_tel["timestamp"] # update state timestamp
            
            msg = {
                "type": "fsw_telemetry",
                "timestamp_ms": int(time.time() * 1000),
                "connected": True,
                "flight_mode": "Standby",
                "telemetry": fsw_tel
            }
            await websocket.send(json.dumps(msg))
            await asyncio.sleep(0.1) # 10 Hz
    except asyncio.CancelledError:
        pass
    except Exception as e:
        print(f"FSW Stream error: {e}")

async def main():
    async with websockets.serve(handler, "0.0.0.0", 9000):
        print("Mock Server started on ws://0.0.0.0:9000")
        await asyncio.Future()  # run forever

if __name__ == "__main__":
    asyncio.run(main())
