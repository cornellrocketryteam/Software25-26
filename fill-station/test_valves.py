#!/usr/bin/env python3
"""
Test WebSocket client for Solenoid Valve control.

Usage:
    python3 test_valves.py

This script connects to the fill station WebSocket server,
and cycles the SV1 and SV2 valves on and off.
"""

import asyncio
import websockets
import json
import sys

async def test_valves():
    uri = "ws://192.168.1.127:9000"
    
    print(f"Connecting to {uri}...")
    
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected!")
            
            # --- TEST SV1 ---
            print("\nTesting SV1 (Chip 1 / Pin 51)...")
            
            # Actuate SV1
            print("Actuating SV1 (ON)...")
            await websocket.send(json.dumps({
                "command": "actuate_valve",
                "valve": "SV1",
                "state": True
            }))
            print(f"Response: {await websocket.recv()}")
            
            await asyncio.sleep(1)
            
            # De-actuate SV1
            print("De-actuating SV1 (OFF)...")
            await websocket.send(json.dumps({
                "command": "actuate_valve",
                "valve": "SV1",
                "state": False
            }))
            print(f"Response: {await websocket.recv()}")


            # --- TEST SV2 ---
            print("\nTesting SV2 (Chip 0 / Pin 34)...")
            
            # Actuate SV2
            print("Actuating SV2 (ON)...")
            await websocket.send(json.dumps({
                "command": "actuate_valve",
                "valve": "SV2",
                "state": True
            }))
            print(f"Response: {await websocket.recv()}")
            
            await asyncio.sleep(1)
            
            # De-actuate SV2
            print("De-actuating SV2 (OFF)...")
            await websocket.send(json.dumps({
                "command": "actuate_valve",
                "valve": "SV2",
                "state": False
            }))
            print(f"Response: {await websocket.recv()}")
            
            print("\nTest Complete!")
            
    except ConnectionRefusedError:
        print("Error: Could not connect to server. Is it running?")
        print("Run 'cargo run' in a separate terminal.")
    except Exception as e:
        print(f"\nError: {e}")

if __name__ == "__main__":
    try:
        asyncio.run(test_valves())
    except KeyboardInterrupt:
        print("\n\nExiting...")
        sys.exit(0)
