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

        # FSW Telemetry
        self.fsw_connected = False
        self.fsw_telemetry = None
        self.fsw_flight_mode = "Unknown"

    def connect(self, url):
        self.url = url
        if self.connected and self.should_run:
            print("Already connected and running.")
            return
        
        # If threads are alive from a previous session, ensure they stop first
        if self.should_run:
            self.disconnect()
            time.sleep(0.5) # Give threads time to die

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
            ws.send(json.dumps({"command": "start_fsw_stream"}))
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

                elif msg_type == "fsw_telemetry":
                    self.fsw_connected = data.get("connected", False)
                    self.fsw_flight_mode = data.get("flight_mode", "Unknown")
                    self.fsw_telemetry = data.get("telemetry", {})

            except Exception as e:
                print(f"Error parsing: {e}")

        def on_close(ws, close_status_code, close_msg):
            print("WebSocket Closed")
            self.connected = False
            # self.ws = None # Don't do this here race condition with run_forever? 
            # Actually, run_forever returns after this.
            # We can leave it, but connected=False should prevent usage.
            
        while self.should_run:
            self.ws = websocket.WebSocketApp(
                self.url,
                on_open=on_open,
                on_message=on_message,
                on_close=on_close
            )
            self.ws.run_forever()
            if self.should_run:
                time.sleep(2)

    def send_command(self, cmd_dict):
        if self.ws and self.connected:
            try:
                self.ws.send(json.dumps(cmd_dict))
            except (websocket.WebSocketConnectionClosedException, BrokenPipeError, ConnectionResetError) as e:
                print(f"Send failed (socket closed): {e}")
                self.connected = False
            except Exception as e:
                print(f"Send failed: {e}")

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
        1. Vent (SV5 High) & Ignite at the same time -> wait 2s
        2. Set SV5 Low -> wait 1s
        3. MAV Open -> wait 7.88s
        4. MAV Close
        5. Set SV1 and SV5 to High, others Low
        """
        def sequence():
            self.launch_status = "Step 1: Venting (SV2_Rocket High) & Firing Igniters..."
            # High = True
            self.send_command({"command": "actuate_valve", "valve": "SV5", "state": True})
            self.update_valve_state_local("SV5", True)
            self.send_command({"command": "ignite"})
            time.sleep(2.0)
            
            self.launch_status = "Step 2: Stopping Vent (SV2_Rocket Low)..."
            # Low = False
            self.send_command({"command": "actuate_valve", "valve": "SV5", "state": False})
            self.update_valve_state_local("SV5", False)
            time.sleep(1.0)
            
            self.launch_status = "Step 3: Opening MAV..."
            self.send_command({"command": "set_mav_angle", "valve": "MAV", "angle": 0.0})
            self.mav["angle"] = 0.0
            time.sleep(7.88)
            
            self.launch_status = "Step 4: Closing MAV & Setting SV1/SV2_Rocket High..."
            self.send_command({"command": "set_mav_angle", "valve": "MAV", "angle": 95.0})
            self.mav["angle"] = 95.0
            
            # Close All (Signal Low = False), EXCEPT SV1 and SV5 which go High
            for sv in ["SV1", "SV2", "SV3", "SV4", "SV5"]:
                if sv in ["SV1", "SV5"]:
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
        client.send_command({"command": "set_mav_angle", "valve": "MAV", "angle": 0.0})
        # Force re-poll
        time.sleep(0.1)
        client.send_command({"command": "get_mav_state", "valve": "MAV"})

    if c2.button("CLOSE", use_container_width=True):
        client.send_command({"command": "set_mav_angle", "valve": "MAV", "angle": 95.0})
        time.sleep(0.1)
        client.send_command({"command": "get_mav_state", "valve": "MAV"})
    
    st.divider()

    st.subheader("Ball Valve Control")
    bvc1, bvc2, bvc3 = st.columns(3)
    
    if bvc1.button("OPEN BV", type="primary", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "low"})
        time.sleep(0.1)
        client.send_command({"command": "bv_signal", "state": "high"})

    if bvc2.button("CLOSE BV", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "low"})
        time.sleep(0.1)
        client.send_command({"command": "bv_signal", "state": "low"})

    if bvc3.button("PAUSE BV", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "high"})

    st.caption("Manual Pins")
    bvs1, bvs2 = st.columns(2)
    if bvs1.button("Signal HIGH", use_container_width=True):
        client.send_command({"command": "bv_signal", "state": "high"})
    if bvs1.button("Signal LOW", use_container_width=True):
        client.send_command({"command": "bv_signal", "state": "low"})

    if bvs2.button("ON_OFF HIGH", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "high"})
    if bvs2.button("ON_OFF LOW", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "low"})

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
            display_name = "SV2_Rocket" if valve == "SV5" else valve
            st.markdown(f"**{display_name}**: :{color}[{label}]")
            
            # Toggle (Uses updated Custom Logic)
            if st.button(f"Toggle", key=f"btn_{valve}"):
                client.toggle_valve_logic(valve)

    st.divider()
    
    st.subheader("Timed Control")
    ct1, ct2, ct3 = st.columns([1, 1, 1])
    target_sv = ct1.selectbox("Valve", valves, format_func=lambda x: "SV2_Rocket" if x == "SV5" else x)
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
        # PT1500_SCALE: 0.909754, PT1500_OFFSET: 5.08926
        # PT2000_SCALE: 1.22124, PT2000_OFFSET: 5.37052
        # LOADCELL_SCALE: 1.69661, LOADCELL_OFFSET: 75.37882
        # Measurement mapping
        if len(adc1) > 0: data.append({"Name": "PT1", "Raw": adc1[0]['raw'], "Scaled": adc1[0]['raw'] * 0.909754 + 5.08926})
        if len(adc1) > 1: data.append({"Name": "PT2", "Raw": adc1[1]['raw'], "Scaled": adc1[1]['raw'] * 0.6125 + 5.0})
        if len(adc1) > 2: data.append({"Name": "PT3", "Raw": adc1[2]['raw'], "Scaled": adc1[2]['raw'] * 0.909754 + 5.0892})
        if len(adc1) > 3: data.append({"Name": "", "Raw": adc1[3]['raw'], "Scaled": adc1[3]['raw'] * 1.22124 + 5.37052})
        
        if len(adc2) > 0: data.append({"Name": "", "Raw": adc2[0]['raw'], "Scaled": adc2[0]['raw'] * 1.22124 + 5.37052})
        if len(adc2) > 1:
            lc_raw = adc2[1]['raw']
            if lc_raw <= 800:
                data.append({"Name": "Load Cell", "Raw": lc_raw, "Scaled": lc_raw * 0.264 - 14.9})

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

# --- FLIGHT SOFTWARE TELEMETRY ---
st.divider()
st.subheader("Flight Software (Umbilical)")

fsw_status = "Connected" if client.fsw_connected else "Disconnected"
fsw_color = "green" if client.fsw_connected else "red"
st.markdown(f"**Umbilical:** :{fsw_color}[{fsw_status}]  |  **Flight Mode:** {client.fsw_flight_mode}")

if client.fsw_telemetry and client.fsw_connected:
    t = client.fsw_telemetry

    fsw_col1, fsw_col2, fsw_col3, fsw_col4 = st.columns(4)

    with fsw_col1:
        st.metric("Altitude", f"{t.get('altitude', 0):.1f} m")
        st.metric("Pressure", f"{t.get('pressure', 0):.1f} Pa")
        st.metric("Temperature", f"{t.get('temp', 0):.1f} C")

    with fsw_col2:
        st.metric("Latitude", f"{t.get('latitude', 0):.6f}")
        st.metric("Longitude", f"{t.get('longitude', 0):.6f}")
        st.metric("Satellites", f"{t.get('num_satellites', 0)}")

    with fsw_col3:
        imu_data = [
            {"Axis": "X", "Accel (m/s2)": t.get('accel_x', 0), "Gyro (deg/s)": t.get('gyro_x', 0), "Mag (uT)": t.get('mag_x', 0)},
            {"Axis": "Y", "Accel (m/s2)": t.get('accel_y', 0), "Gyro (deg/s)": t.get('gyro_y', 0), "Mag (uT)": t.get('mag_y', 0)},
            {"Axis": "Z", "Accel (m/s2)": t.get('accel_z', 0), "Gyro (deg/s)": t.get('gyro_z', 0), "Mag (uT)": t.get('mag_z', 0)},
        ]
        st.caption("IMU")
        st.dataframe(pd.DataFrame(imu_data), hide_index=True, use_container_width=True)

    with fsw_col4:
        st.metric("PT3 (ADC)", f"{t.get('pt3', 0):.2f}")
        st.metric("PT4 (ADC)", f"{t.get('pt4', 0):.2f}")
        st.metric("RTD (ADC)", f"{t.get('rtd', 0):.2f}")

# FSW Command Buttons
st.caption("FSW Commands")
fsw_btn_cols = st.columns(9)
if fsw_btn_cols[0].button("FSW Launch", type="primary", use_container_width=True):
    client.send_command({"command": "fsw_launch"})
if fsw_btn_cols[1].button("FSW Open MAV", use_container_width=True):
    client.send_command({"command": "fsw_open_mav"})
if fsw_btn_cols[2].button("FSW Close MAV", use_container_width=True):
    client.send_command({"command": "fsw_close_mav"})
if fsw_btn_cols[3].button("FSW Open SV", use_container_width=True):
    client.send_command({"command": "fsw_open_sv"})
if fsw_btn_cols[4].button("FSW Close SV", use_container_width=True):
    client.send_command({"command": "fsw_close_sv"})
if fsw_btn_cols[5].button("FSW Safe", use_container_width=True):
    client.send_command({"command": "fsw_safe"})
if fsw_btn_cols[6].button("Reset FRAM", use_container_width=True):
    client.send_command({"command": "fsw_reset_fram"})
if fsw_btn_cols[7].button("Reset Card", use_container_width=True):
    client.send_command({"command": "fsw_reset_card"})
if fsw_btn_cols[8].button("FSW Reboot", use_container_width=True):
    client.send_command({"command": "fsw_reboot"})

st.rerun()
