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
            "SV1": {"open": False, "continuity": False},
        }
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
                    self.send_command({"command": "get_valve_state", "valve": "SV1"})
                    
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
            self.send_command({"command": "get_valve_state", "valve": "SV1"})
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
                    valve_name = data.get("valve")
                    if valve_name and valve_name in self.valves:
                        self.valves[valve_name]["open"] = data.get("open", False)
                        self.valves[valve_name]["continuity"] = data.get("continuity", False)

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
            self.valves[valve]["open"] = state

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
        current_state = self.valves[valve]["open"]
        
        target_state = False
        if valve == "SV5":
            # "If I query state and it says open... to toggle I must send another open command"
            # Open = True. So if current=True, send True.
            target_state = current_state 
        else:
            # Standard toggle
            target_state = not current_state

        self.send_command({"command": "actuate_valve", "valve": valve, "open": target_state})
        
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
        3. Set SV1 and SV5 to High, others Low
        """
        def sequence():
            self.launch_status = "Step 1: Venting (FSW SV Open) & Firing Igniters..."
            self.send_command({"command": "fsw_open_sv"})
            self.send_command({"command": "ignite"})
            time.sleep(2.0)

            self.launch_status = "Step 2: Stopping Vent (FSW SV Close)..."
            self.send_command({"command": "fsw_close_sv"})
            time.sleep(1.0)

            # Open SV1 and FSW SV
            self.send_command({"command": "actuate_valve", "valve": "SV1", "open": True})
            self.update_valve_state_local("SV1", True)
            self.send_command({"command": "fsw_open_sv"})

            self.launch_status = "Sequence Complete. Verifying States..."
            time.sleep(1.0)
            self.launch_status = None

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

# --- LEFT: Ball Valve & Igniters ---
with col_left:
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

    st.subheader("QD Stepper")
    qd1, qd2 = st.columns(2)
    if qd1.button("QD RETRACT", type="primary", use_container_width=True):
        client.send_command({"command": "qd_retract"})
    if qd2.button("QD EXTEND", use_container_width=True):
        client.send_command({"command": "qd_extend"})

    st.caption("Manual Step")
    qd_steps = st.number_input("Steps", min_value=1, value=200, step=1, key="qd_steps")
    qd_dir_col1, qd_dir_col2 = st.columns(2)
    if qd_dir_col1.button("Step CW", use_container_width=True):
        client.send_command({"command": "qd_move", "steps": qd_steps, "direction": True})
    if qd_dir_col2.button("Step CCW", use_container_width=True):
        client.send_command({"command": "qd_move", "steps": qd_steps, "direction": False})

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

    is_open = client.valves["SV1"]["open"]
    color = "green" if is_open else "red"
    label = "OPEN" if is_open else "CLOSED"
    st.markdown(f"**SV1**: :{color}[{label}]")

    if st.button("Toggle SV1", key="btn_SV1"):
        client.toggle_valve_logic("SV1")

    st.divider()

    st.subheader("Timed Control")
    ct1, ct2 = st.columns([1, 1])
    duration = ct1.number_input("Seconds", min_value=0.1, value=1.0, step=0.1)
    if ct2.button("Pulse SV1", use_container_width=True):
        client.run_timed_actuation("SV1", duration)
        st.toast(f"Pulsing SV1 for {duration}s")

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
        PT1000_SCALE, PT1000_OFFSET = 0.6125, 5.0
        PT1500_SCALE, PT1500_OFFSET = 0.909754, 5.08926
        
        # Original reference comments:
        # PT1500_SCALE: 0.909754, PT1500_OFFSET: 5.08926
        # PT2000_SCALE: 1.22124, PT2000_OFFSET: 5.37052
        # LOADCELL_SCALE: 1.69661, LOADCELL_OFFSET: 75.37882
        # Measurement mapping
        if len(adc1) > 0: data.append({"Name": "PT1", "Raw": adc1[0]['raw'], "Scaled": adc1[0]['raw'] * PT1500_SCALE + PT1500_OFFSET})
        if len(adc1) > 1: data.append({"Name": "PT2", "Raw": adc1[1]['raw'], "Scaled": adc1[1]['raw'] * PT1000_SCALE + PT1000_OFFSET})
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
        sv_state = "OPEN" if t.get('sv_open', False) else "CLOSED"
        sv_color = "green" if t.get('sv_open', False) else "red"
        mav_state = "OPEN" if t.get('mav_open', False) else "CLOSED"
        mav_color = "green" if t.get('mav_open', False) else "red"
        st.markdown(f"**FSW SV:** :{sv_color}[{sv_state}]")
        st.markdown(f"**FSW MAV:** :{mav_color}[{mav_state}]")

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
