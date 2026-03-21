#!/usr/bin/env python3
"""
Test WebSocket client for ADC streaming from fill station.

Usage:
    python3 test_adc_stream.py

This script connects to the fill station WebSocket server and starts streaming
ADC data. Press Ctrl+C to stop.
"""

import asyncio
import websockets
import json
import sys
from datetime import datetime


async def test_adc_stream():
    uri = "ws://192.168.1.127:9000"
    
    print(f"Connecting to {uri}...")
    
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected!")
            
            # Send start streaming command
            start_cmd = {"command": "start_adc_stream"}
            await websocket.send(json.dumps(start_cmd))
            print("Sent start_adc_stream command")
            
            # Receive the Success response
            response = await websocket.recv()
            print(f"Response: {response}\n")
            
            print("Receiving ADC data stream (press Ctrl+C to stop)...\n")
            print("=" * 120)
            
            sample_count = 0
            
            while True:
                # Receive ADC data
                data = await websocket.recv()
                msg = json.loads(data)
                
                if msg.get("type") == "adc_data":
                    sample_count += 1
                    
                    # Print header every 20 samples
                    if sample_count % 20 == 1:
                        print(f"\n{'Sample':<8} {'Timestamp':<20} {'Valid':<7} ADC1 Ch0-3 (Raw/V/Scaled)                           ADC2 Ch0-3 (Raw/V)")
                        print("-" * 120)
                    
                    # Format timestamp
                    ts = datetime.fromtimestamp(msg["timestamp_ms"] / 1000.0)
                    ts_str = ts.strftime("%H:%M:%S.%f")[:-3]
                    
                    # Extract ADC1 data
                    adc1 = msg["adc1"]
                    adc1_ch0 = f"{adc1[0]['raw']:5d}/{adc1[0]['voltage']:6.3f}/{adc1[0]['scaled']:6.2f}" if adc1[0]['scaled'] else f"{adc1[0]['raw']:5d}/{adc1[0]['voltage']:6.3f}"
                    adc1_ch1 = f"{adc1[1]['raw']:5d}/{adc1[1]['voltage']:6.3f}/{adc1[1]['scaled']:6.2f}" if adc1[1]['scaled'] else f"{adc1[1]['raw']:5d}/{adc1[1]['voltage']:6.3f}"
                    adc1_ch2 = f"{adc1[2]['raw']:5d}/{adc1[2]['voltage']:6.3f}"
                    adc1_ch3 = f"{adc1[3]['raw']:5d}/{adc1[3]['voltage']:6.3f}"
                    
                    # Extract ADC2 data
                    adc2 = msg["adc2"]
                    adc2_str = " | ".join([f"{ch['raw']:5d}/{ch['voltage']:6.3f}" for ch in adc2])
                    
                    # Print row
                    valid_str = "✓" if msg["valid"] else "✗"
                    print(f"{sample_count:<8d} {ts_str:<20} {valid_str:<7} {adc1_ch0} | {adc1_ch1} | {adc1_ch2} | {adc1_ch3} | {adc2_str}")
                
    except websockets.exceptions.ConnectionClosed:
        print("\nConnection closed by server")
    except KeyboardInterrupt:
        print("\n\nStopping stream...")
        # Send stop command (best effort)
        try:
            stop_cmd = {"command": "stop_adc_stream"}
            await websocket.send(json.dumps(stop_cmd))
            await websocket.close()
        except:
            pass
    except Exception as e:
        print(f"\nError: {e}")


if __name__ == "__main__":
    try:
        asyncio.run(test_adc_stream())
    except KeyboardInterrupt:
        print("\n\nExiting...")
        sys.exit(0)
