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
    "igniters": {
        1: {"continuity": True},
        2: {"continuity": True},
    },
    "adc_stream_active": False
}

async def handler(websocket):
    print(f"Client connected: {websocket.remote_address}")
    try:
        # Start a background task for ADC streaming if active
        adc_task = None
        
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
                state["mav"]["pulse_width_us"] = 1000 + (angle / 90.0) * 1000
            
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
                # The real API uses "id" field in request?
                # "Format: {"command": "get_igniter_continuity", "id": 1}"
                # But wait, docs say response has ID but request format just has ID.
                # Let's handle both generic and specific if needed.
                # Actually docs say: {"command": "get_igniter_continuity", "id": 1}
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
                # Fire!
                # In a real system this sends success immediately.
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
                pass
            elif command == "bv_close":
                pass
            
            await websocket.send(json.dumps(response))

    except websockets.exceptions.ConnectionClosed:
        print("Client disconnected")
    finally:
        state["adc_stream_active"] = False


async def stream_adc(websocket):
    start_time = time.time()
    try:
        while state["adc_stream_active"]:
            # Generate fake data
            # ADC1: 4 channels. ADC2: 4 channels
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
                    {"raw": random.randint(2000, 2047), "voltage": 3.3, "scaled": None}, # Maybe battery?
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                    {"raw": 0, "voltage": 0.0, "scaled": None},
                ]
            }
            await websocket.send(json.dumps(msg))
            await asyncio.sleep(0.1) # 10 Hz
    except asyncio.CancelledError:
        pass
    except Exception as e:
        print(f"Stream error: {e}")

async def main():
    async with websockets.serve(handler, "0.0.0.0", 9000):
        print("Mock Server started on ws://0.0.0.0:9000")
        await asyncio.Future()  # run forever

if __name__ == "__main__":
    asyncio.run(main())
