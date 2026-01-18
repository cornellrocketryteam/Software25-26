import streamlit as st
import websocket
import threading
import json
import time
import pandas as pd
from collections import deque

# --- Configuration ---
st.set_page_config(
    page_title="Fill Station Dashboard",
    page_icon="🚀",
    layout="wide",
)

# --- Singleton WebSocket Client ---
class FillStationClient:
    def __init__(self):
        self.ws = None
        self.url = "ws://localhost:9000"
        self.connected = False
        self.thread = None
        self.hb_thread = None
        self.poll_thread = None
        self.should_run = False
        
        # Data Store
        self.latest_adc = None
        self.valves = {
            f"SV{i}": {"actuated": False, "continuity": False} for i in range(1, 6)
        }
        self.mav = {"angle": 0.0, "pulse_width_us": 0}
        self.igniters = {1: False, 2: False}
        self.last_update = time.time()
        self.launch_status = None # For UI Banner

    def connect(self, url):
        self.url = url
        if self.connected:
            return
        
        self.should_run = True
        self.thread = threading.Thread(target=self._run_ws, daemon=True)
        self.thread.start()
        
        self.hb_thread = threading.Thread(target=self._heartbeat_loop, daemon=True)
        self.hb_thread.start()
        
        self.poll_thread = threading.Thread(target=self._polling_loop, daemon=True)
        self.poll_thread.start()

    def disconnect(self):
        self.should_run = False
        if self.ws:
            self.ws.close()
        self.connected = False

    def _heartbeat_loop(self):
        while self.should_run:
            if self.connected:
                try:
                    self.send_command({"command": "heartbeat"})
                except Exception as e:
                    print(f"Heartbeat failed: {e}")
            time.sleep(5)

    def _polling_loop(self):
        """Query state every 3 seconds"""
        while self.should_run:
            if self.connected:
                try:
                    # Poll Valves
                    for val in ["SV1", "SV2", "SV3", "SV4", "SV5"]:
                        self.send_command({"command": "get_valve_state", "valve": val})
                        time.sleep(0.05) # Spacer
                    
                    # Poll MAV
                    self.send_command({"command": "get_mav_state", "valve": "MAV"})
                    
                    # Poll Igniters
                    self.send_command({"command": "get_igniter_continuity", "id": 1})
                    self.send_command({"command": "get_igniter_continuity", "id": 2})
                    
                except Exception as e:
                    print(f"Polling failed: {e}")
            time.sleep(3)

    def _run_ws(self):
        def on_open(ws):
            self.connected = True
            ws.send(json.dumps({"command": "start_adc_stream"}))
            # Initial Poll
            for val in ["SV1", "SV2", "SV3", "SV4", "SV5"]:
                self.send_command({"command": "get_valve_state", "valve": val})
                time.sleep(0.02)
            self.send_command({"command": "get_mav_state", "valve": "MAV"})
            self.send_command({"command": "get_igniter_continuity", "id": 1})
            self.send_command({"command": "get_igniter_continuity", "id": 2})

        def on_message(ws, message):
            self.last_update = time.time()
            try:
                data = json.loads(message)
                msg_type = data.get("type")

                if msg_type == "adc_data":
                    self.latest_adc = data
                
                elif msg_type == "valve_state":
                    # Now utilizing the 'valve' identifier from updated API
                    valve_name = data.get("valve")
                    if valve_name and valve_name in self.valves:
                        self.valves[valve_name]["actuated"] = data.get("actuated", False)
                        self.valves[valve_name]["continuity"] = data.get("continuity", False) 

                elif msg_type == "mav_state":
                    self.mav["angle"] = data.get("angle", 0)
                    self.mav["pulse_width_us"] = data.get("pulse_width_us", 0)

                elif msg_type == "igniter_continuity":
                    ign_id = data.get("id")
                    if ign_id:
                        self.igniters[ign_id] = data.get("continuity", False)

            except Exception as e:
                print(f"Error parsing: {e}")

        def on_close(ws, close_status_code, close_msg):
            self.connected = False
            if self.should_run:
                time.sleep(2)
                self._run_ws()

        self.ws = websocket.WebSocketApp(
            self.url,
            on_open=on_open,
            on_message=on_message,
            on_close=on_close
        )
        self.ws.run_forever()

    def send_command(self, cmd_dict):
        if self.ws and self.connected:
            self.ws.send(json.dumps(cmd_dict))

    def update_valve_state_local(self, valve, state):
        if valve in self.valves:
            self.valves[valve]["actuated"] = state

    def toggle_valve_logic(self, valve):
        """
        Custom Toggle Logic:
        1. Query State (we rely on cached state from poll or update it now)
        2. Determind Command:
           - SV5: cmd = current_state (Funky Logic: Open->Open toggles)
           - Others: cmd = !current_state
        3. Send Actuate
        4. Poll again
        """
        # We use the cached state which is updated by poll/actuate
        current_state = self.valves[valve]["actuated"]
        
        target_state = False
        if valve == "SV5":
            # "If I query state and it says open... to toggle I must send another open command"
            # Open = True. So if current=True, send True.
            target_state = current_state 
        else:
            # Standard toggle
            target_state = not current_state

        self.send_command({"command": "actuate_valve", "valve": valve, "state": target_state})
        
        # We assume succesful toggle implies state flip for standard, 
        # but for SV5 "sending Open to Open" toggles it... so does the state become Closed?
        # User said: "If I query... open... send open command... to toggle"
        # Toggle means state changes. So we optimistically flip the local state check?
        # Actually, polling will fix it in 3s, but for UI responsiveness:
        self.update_valve_state_local(valve, not current_state)
        
        # Trigger immediate re-poll
        time.sleep(0.1)
        self.send_command({"command": "get_valve_state", "valve": valve})


    # --- SEQUENCES ---
    
    def run_timed_actuation(self, valve, duration):
        """Runs in a background thread"""
        def sequence():
            self.toggle_valve_logic(valve) # Initial Toggle
            time.sleep(0.2)
            time.sleep(duration)
            self.toggle_valve_logic(valve) # Toggle Back
        
        threading.Thread(target=sequence, daemon=True).start()

    def run_vent_ignite_launch(self):
        """
        Vent Ignite Launch:
        1. SV5 Low -> 1s -> SV5 High
        2. Fire Igniters
        3. Wait 4s -> MAV Open
        4. Wait 7.88s -> MAV Close
        5. Set all SVs Low (actuate=False)
        """
        def sequence():
            self.launch_status = "Step 1: Setting SV5 Signal LOW..."
            # Low = False (based on Step 8 comment correction)
            self.send_command({"command": "actuate_valve", "valve": "SV5", "state": False})
            self.update_valve_state_local("SV5", False) 
            time.sleep(1.0)
            
            self.launch_status = "Step 2: Setting SV5 Signal HIGH & Firing Igniters..."
            # High = True
            self.send_command({"command": "actuate_valve", "valve": "SV5", "state": True})
            self.update_valve_state_local("SV5", True)
            self.send_command({"command": "ignite"})
            time.sleep(4.0)
            
            self.launch_status = "Step 3: Opening MAV..."
            self.send_command({"command": "mav_open", "valve": "MAV"})
            self.mav["angle"] = 90.0
            time.sleep(7.88)
            
            self.launch_status = "Step 4: Closing MAV & Setting All SVs LOW..."
            self.send_command({"command": "mav_close", "valve": "MAV"})
            self.mav["angle"] = 0.0
            
            # Close All (Signal Low = False)
            for sv in ["SV1", "SV2", "SV3", "SV4", "SV5"]:
                if sv=="SV1":
                    self.send_command({"command": "actuate_valve", "valve": sv, "state": True})
                    self.update_valve_state_local(sv, True)
                else:
                    self.send_command({"command": "actuate_valve", "valve": sv, "state": False})
                    self.update_valve_state_local(sv, False)
            
            # Repoll everything
            self.launch_status = "Sequence Complete. Verifying States..."
            time.sleep(1.0)
            self.launch_status = None # Clear Banner

        threading.Thread(target=sequence, daemon=True).start()


# --- Global State ---
@st.cache_resource
def get_client_v3():
    return FillStationClient()

client = get_client_v3()

# --- UI Layout ---

# Sidebar
with st.sidebar:
    st.header("Connection")
    url = st.text_input("Server URL", value="ws://localhost:9000")
    if st.button("Connect"): client.connect(url)
    if st.button("Disconnect"): client.disconnect()
    status = "Connected" if client.connected else "Disconnected"
    st.markdown(f"Status: **{status}**")

if not client.connected:
    st.warning("Connect to server to view dashboard.")
    st.stop()

# Auto Refresh
if 'last_refresh' not in st.session_state: st.session_state.last_refresh = time.time()
time.sleep(0.1)

# Status Banner  
ls = getattr(client, "launch_status", None)
if ls:
    st.warning(f"🚀 **LAUNCH SEQUENCE**: {ls}")

# V2 Layout: Left (MAV/Ign), Middle (SV), Right (ADC)
col_left, col_mid, col_right = st.columns([1, 2, 2])

# --- LEFT: MAV & Igniters ---
with col_left:
    st.subheader("MAV Control")
    st.metric("Angle", f"{client.mav.get('angle', 0):.1f}°")
    
    c1, c2 = st.columns(2)
    if c1.button("OPEN", type="primary", use_container_width=True):
        client.send_command({"command": "mav_open", "valve": "MAV"})
        # Force re-poll
        time.sleep(0.1)
        client.send_command({"command": "get_mav_state", "valve": "MAV"})

    if c2.button("CLOSE", use_container_width=True):
        client.send_command({"command": "mav_close", "valve": "MAV"})
        time.sleep(0.1)
        client.send_command({"command": "get_mav_state", "valve": "MAV"})
    
    st.divider()
    
    st.subheader("Igniters")
    i1 = client.igniters.get(1, False)
    i2 = client.igniters.get(2, False)
    
    st.markdown(f"**Igniter 1:** {'✅ Continuity' if i1 else '❌ OPEN'}")
    st.markdown(f"**Igniter 2:** {'✅ Continuity' if i2 else '❌ OPEN'}")
    
    if st.button("Query Continuity", use_container_width=True):
        client.send_command({"command": "get_igniter_continuity", "id": 1})
        client.send_command({"command": "get_igniter_continuity", "id": 2})

    if st.button("FIRE IGNITERS", type="primary", use_container_width=True):
        client.send_command({"command": "ignite"})

# --- MIDDLE: Solenoids & Automation ---
with col_mid:
    st.subheader("Solenoid Valves")
    
    sv_cols = st.columns(3)
    valves = ["SV1", "SV2", "SV3", "SV4", "SV5"]
    for i, valve in enumerate(valves):
        with sv_cols[i % 3]:
            # Indicator
            is_open = client.valves[valve]["actuated"]
            color = "green" if is_open else "red"
            label = "OPEN" if is_open else "CLOSED"
            st.markdown(f"**{valve}**: :{color}[{label}]")
            
            # Toggle (Uses updated Custom Logic)
            if st.button(f"Toggle", key=f"btn_{valve}"):
                client.toggle_valve_logic(valve)

    st.divider()
    
    st.subheader("Timed Control")
    ct1, ct2, ct3 = st.columns([1, 1, 1])
    target_sv = ct1.selectbox("Valve", valves)
    duration = ct2.number_input("Seconds", min_value=0.1, value=1.0, step=0.1)
    if ct3.button("Pulse Valve", use_container_width=True):
        client.run_timed_actuation(target_sv, duration)
        st.toast(f"Pulsing {target_sv} for {duration}s")

    st.divider()
    
    st.subheader("Launch Sequence")
    if st.button("🚀 VENT IGNITE LAUNCH", type="primary", use_container_width=True):
        client.run_vent_ignite_launch()

# --- RIGHT: ADC Monitoring ---
with col_right:
    st.subheader("Sensor Data")
    
    if client.latest_adc:
        data = []
        adc1 = client.latest_adc.get("adc1", [])
        adc2 = client.latest_adc.get("adc2", [])
        
        # Mapping Schema
        if len(adc1) > 0: data.append({"Name": "PT5", "Raw": adc1[0]['raw'], "Scaled": adc1[0]['scaled']})
        if len(adc1) > 1: data.append({"Name": "PT2", "Raw": adc1[1]['raw'], "Scaled": adc1[1]['scaled']})
        if len(adc1) > 2: data.append({"Name": "PT7", "Raw": adc1[2]['raw'], "Scaled": adc1[2]['scaled']})
        if len(adc1) > 3: data.append({"Name": "PT8", "Raw": adc1[3]['raw'], "Scaled": adc1[3]['scaled']})
        
        if len(adc2) > 0: data.append({"Name": "PT6", "Raw": adc2[0]['raw'], "Scaled": adc2[0]['scaled']})
        if len(adc2) > 1: data.append({"Name": "Load Cell", "Raw": adc2[1]['raw'], "Scaled": adc2[1]['scaled']})

        df = pd.DataFrame(data)
        st.dataframe(
            df, 
            column_config={
                "Scaled": st.column_config.NumberColumn(format="%.2f"),
                "Raw": st.column_config.NumberColumn(format="%d"),
            },
            hide_index=True,
            use_container_width=True
        )
    else:
        st.info("Waiting for data...")

st.rerun()
