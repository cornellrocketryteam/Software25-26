#!/usr/bin/env python3
import math
import os
import urllib.request
import time

# --- CONFIGURATION ---
# Replace these with the bounding box of your launch site!
# Example: Spaceport America, NM
LAT_MIN = 32.90
LAT_MAX = 33.00
LON_MIN = -107.00
LON_MAX = -106.90

MIN_ZOOM = 12
MAX_ZOOM = 16

OUTPUT_DIR = os.path.join(os.path.dirname(__file__), '..', 'public', 'tiles')
# ---------------------

def deg2num(lat_deg, lon_deg, zoom):
    lat_rad = math.radians(lat_deg)
    n = 2.0 ** zoom
    xtile = int((lon_deg + 180.0) / 360.0 * n)
    ytile = int((1.0 - math.asinh(math.tan(lat_rad)) / math.pi) / 2.0 * n)
    return (xtile, ytile)

def download_tiles():
    headers = {
        'User-Agent': 'GroundStationUI-OfflineCache/1.0'
    }

    for z in range(MIN_ZOOM, MAX_ZOOM + 1):
        x_min, y_max = deg2num(LAT_MIN, LON_MIN, z)
        x_max, y_min = deg2num(LAT_MAX, LON_MAX, z)
        
        # Depending on hemisphere, min/max might be flipped
        if x_min > x_max: x_min, x_max = x_max, x_min
        if y_min > y_max: y_min, y_max = y_max, y_min

        print(f"Zoom {z}: {x_max - x_min + 1} x {y_max - y_min + 1} tiles")

        for x in range(x_min, x_max + 1):
            for y in range(y_min, y_max + 1):
                url = f"https://tile.openstreetmap.org/{z}/{x}/{y}.png"
                target_path = os.path.join(OUTPUT_DIR, str(z), str(x), f"{y}.png")
                
                if os.path.exists(target_path):
                    continue

                os.makedirs(os.path.dirname(target_path), exist_ok=True)
                
                print(f"Downloading {url} -> {target_path}")
                req = urllib.request.Request(url, headers=headers)
                try:
                    with urllib.request.urlopen(req) as response:
                        with open(target_path, 'wb') as out_file:
                            out_file.write(response.read())
                except Exception as e:
                    print(f"Failed to download {url}: {e}")
                
                # Be nice to OSM servers
                time.sleep(0.1)

if __name__ == '__main__':
    print("Starting map tile download...")
    download_tiles()
    print("Done! You can now use offline maps.")
