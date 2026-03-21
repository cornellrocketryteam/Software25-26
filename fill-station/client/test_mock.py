import asyncio
import websockets
import json

async def test():
    uri = "ws://localhost:9000"
    async with websockets.connect(uri) as websocket:
        # Test 1: Get Valve State
        await websocket.send(json.dumps({"command": "get_valve_state", "valve": "SV1"}))
        resp = await websocket.recv()
        print(f"Response 1: {resp}")
        assert "valve_state" in resp

        # Test 2: Start ADC Stream
        await websocket.send(json.dumps({"command": "start_adc_stream"}))
        
        # Consume Success Response
        ack = await websocket.recv()
        print(f"Stream Ack: {ack}")
        assert "success" in ack

        for _ in range(3):
            resp = await websocket.recv()
            print(f"ADC Stream: {resp}")
            assert "adc_data" in resp

        # Test 3: Stop ADC Stream
        await websocket.send(json.dumps({"command": "stop_adc_stream"}))
        
        print("Verification Successful")

asyncio.run(test())
