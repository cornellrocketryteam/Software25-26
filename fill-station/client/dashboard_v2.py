import streamlit as st
import websocket
import threading
import json
import time
import pandas as pd

# --- Configuration ---
st.set_page_config(
    page_title="Fill Station Dashboard v2",
    layout="wide",
)

# --- WebSocket Client ---
class FillStationClient:
    def __init__(self):
        self.ws = None
        self.url = "ws://localhost:9000"
        self.connected = False
        self.thread = None
        self.hb_thread = None
        self.poll_thread = None
        self.should_run = False

        # SV1 state
        self.sv1_open = False
        self.sv1_continuity = False

        # Igniters
        self.igniters = {1: False, 2: False}

        # ADC data
        self.latest_adc = None

        # FSW telemetry
        self.fsw_connected = False
        self.fsw_flight_mode = "Unknown"
        self.fsw_telemetry = {}

        # Launch status banner
        self.launch_status = None

    def connect(self, url):
        self.url = url
        if self.connected and self.should_run:
            return
        if self.should_run:
            self.disconnect()
            time.sleep(0.5)

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
                except Exception:
                    pass
            time.sleep(5)

    def _polling_loop(self):
        while self.should_run:
            if self.connected:
                try:
                    self.send_command({"command": "get_valve_state", "valve": "SV1"})
                    time.sleep(0.05)
                    self.send_command({"command": "get_igniter_continuity", "id": 1})
                    time.sleep(0.05)
                    self.send_command({"command": "get_igniter_continuity", "id": 2})
                except Exception:
                    pass
            time.sleep(3)

    def _run_ws(self):
        def on_open(ws):
            self.connected = True
            ws.send(json.dumps({"command": "start_adc_stream"}))
            ws.send(json.dumps({"command": "start_fsw_stream"}))
            self.send_command({"command": "get_valve_state", "valve": "SV1"})
            time.sleep(0.02)
            self.send_command({"command": "get_igniter_continuity", "id": 1})
            self.send_command({"command": "get_igniter_continuity", "id": 2})

        def on_message(ws, message):
            try:
                data = json.loads(message)
                msg_type = data.get("type")

                if msg_type == "adc_data":
                    self.latest_adc = data

                elif msg_type == "valve_state":
                    valve = data.get("valve")
                    if valve == "SV1":
                        self.sv1_open = data.get("open", False)
                        self.sv1_continuity = data.get("continuity", False)

                elif msg_type == "igniter_continuity":
                    ign_id = data.get("id")
                    if ign_id in (1, 2):
                        self.igniters[ign_id] = data.get("continuity", False)

                elif msg_type == "fsw_telemetry":
                    self.fsw_connected = data.get("connected", False)
                    self.fsw_flight_mode = data.get("flight_mode", "Unknown")
                    self.fsw_telemetry = data.get("telemetry", {})

            except Exception as e:
                print(f"Error parsing message: {e}")

        def on_close(ws, close_status_code, close_msg):
            self.connected = False

        while self.should_run:
            self.ws = websocket.WebSocketApp(
                self.url,
                on_open=on_open,
                on_message=on_message,
                on_close=on_close,
            )
            self.ws.run_forever()
            if self.should_run:
                time.sleep(2)

    def send_command(self, cmd_dict):
        if self.ws and self.connected:
            try:
                self.ws.send(json.dumps(cmd_dict))
            except (websocket.WebSocketConnectionClosedException, BrokenPipeError, ConnectionResetError):
                self.connected = False
            except Exception as e:
                print(f"Send failed: {e}")

    # --- Sequences ---
    def run_sv2_timed_actuation(self, duration):
        """Open SV2-Rocket for a set duration then close it."""
        def sequence():
            self.launch_status = f"SV2-Rocket timed actuation: OPEN for {duration}s..."
            self.send_command({"command": "fsw_open_sv"})
            time.sleep(duration)
            self.send_command({"command": "fsw_close_sv"})
            self.launch_status = None
        threading.Thread(target=sequence, daemon=True).start()

    def run_launch(self):
        """Fire igniters and send FSW launch simultaneously."""
        def sequence():
            self.launch_status = "LAUNCH: Firing igniters + FSW Launch..."
            self.send_command({"command": "ignite"})
            self.send_command({"command": "fsw_launch"})
            time.sleep(3)
            self.launch_status = None
        threading.Thread(target=sequence, daemon=True).start()

    def run_vent_ignite_launch_2s(self):
        """2 Sec Launch: Open SV2 + ignite, wait 2s, close SV2, wait 2s, open MAV 7.88s, close MAV."""
        def sequence():
            self.launch_status = "2s LAUNCH: Opening SV2 + Igniting..."
            self.send_command({"command": "fsw_open_sv"})
            self.send_command({"command": "ignite"})
            time.sleep(2)
            self.launch_status = "2s LAUNCH: Closing SV2..."
            self.send_command({"command": "fsw_close_sv"})
            time.sleep(2)
            self.launch_status = "2s LAUNCH: MAV OPEN (7.88s)..."
            self.send_command({"command": "fsw_open_mav"})
            time.sleep(7.88)
            self.send_command({"command": "fsw_close_mav"})
            self.launch_status = None
        threading.Thread(target=sequence, daemon=True).start()

    def run_vent_ignite_launch_1s(self):
        """1 Sec Launch: Open SV2 + ignite, wait 2s, close SV2, wait 1s, open MAV 7.88s, close MAV."""
        def sequence():
            self.launch_status = "1s LAUNCH: Opening SV2 + Igniting..."
            self.send_command({"command": "fsw_open_sv"})
            self.send_command({"command": "ignite"})
            time.sleep(2)
            self.launch_status = "1s LAUNCH: Closing SV2..."
            self.send_command({"command": "fsw_close_sv"})
            time.sleep(1)
            self.launch_status = "1s LAUNCH: MAV OPEN (7.88s)..."
            self.send_command({"command": "fsw_open_mav"})
            time.sleep(7.88)
            self.send_command({"command": "fsw_close_mav"})
            self.launch_status = None
        threading.Thread(target=sequence, daemon=True).start()


# --- Singleton Client ---
@st.cache_resource
def get_client():
    return FillStationClient()

client = get_client()

# --- Sidebar: Connection ---
with st.sidebar:
    st.header("Connection")
    url = st.text_input("Server URL", value="ws://localhost:9000")
    if st.button("Connect"):
        client.connect(url)
    if st.button("Disconnect"):
        client.disconnect()
    status = "Connected" if client.connected else "Disconnected"
    color = "green" if client.connected else "red"
    st.markdown(f"Status: :{color}[**{status}**]")

if not client.connected:
    st.warning("Connect to server to view dashboard.")
    st.stop()

# Auto-refresh
time.sleep(0.1)

# Status banner
if client.launch_status:
    st.warning(f"**ACTIVE**: {client.launch_status}")

# ==========================================
# MAIN LAYOUT: 3 columns
# Left: SV1, Ball Valve, QD Stepper
# Middle: Igniters, SV2-Rocket, Launch
# Right: Sensors
# ==========================================
col_left, col_mid, col_right = st.columns([1, 1, 1.5])

# --- LEFT COLUMN: SV1 + Ball Valve + QD ---
with col_left:
    # --- SV1 ---
    st.subheader("SV1 (Fill Station)")
    sv1_color = "green" if client.sv1_open else "red"
    sv1_label = "OPEN" if client.sv1_open else "CLOSED"
    cont_txt = "YES" if client.sv1_continuity else "NO"
    st.markdown(f"State: :{sv1_color}[**{sv1_label}**] | Continuity: **{cont_txt}**")

    sv1_c1, sv1_c2, sv1_c3 = st.columns(3)
    if sv1_c1.button("OPEN SV1", type="primary", use_container_width=True):
        client.send_command({"command": "actuate_valve", "valve": "SV1", "open": True})
    if sv1_c2.button("CLOSE SV1", use_container_width=True):
        client.send_command({"command": "actuate_valve", "valve": "SV1", "open": False})
    if sv1_c3.button("Query SV1", use_container_width=True):
        client.send_command({"command": "get_valve_state", "valve": "SV1"})

    st.divider()

    # --- Ball Valve ---
    st.subheader("Ball Valve")
    bv_c1, bv_c2, bv_c3 = st.columns(3)
    if bv_c1.button("OPEN BV", type="primary", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "low"})
        time.sleep(0.1)
        client.send_command({"command": "bv_signal", "state": "high"})
    if bv_c2.button("CLOSE BV", use_container_width=True):
        client.send_command({"command": "bv_on_off", "state": "low"})
        time.sleep(0.1)
        client.send_command({"command": "bv_signal", "state": "low"})
    if bv_c3.button("PAUSE BV", use_container_width=True):
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

    # --- QD Stepper ---
    st.subheader("QD Stepper")
    qd_c1, qd_c2 = st.columns(2)
    if qd_c1.button("QD RETRACT", type="primary", use_container_width=True):
        client.send_command({"command": "qd_retract"})
    if qd_c2.button("QD EXTEND", use_container_width=True):
        client.send_command({"command": "qd_extend"})

    st.caption("Manual Steps")
    qd_steps = st.number_input("Steps", min_value=1, value=200, step=1, key="qd_steps")
    qd_mc1, qd_mc2 = st.columns(2)
    if qd_mc1.button("Step CW (Retract)", use_container_width=True):
        client.send_command({"command": "qd_move", "steps": qd_steps, "direction": True})
    if qd_mc2.button("Step CCW (Extend)", use_container_width=True):
        client.send_command({"command": "qd_move", "steps": qd_steps, "direction": False})


# --- MIDDLE COLUMN: Igniters + SV2-Rocket + Launch ---
with col_mid:
    # --- Igniters ---
    st.subheader("Igniters")
    i1 = client.igniters.get(1, False)
    i2 = client.igniters.get(2, False)
    i1_color = "green" if i1 else "red"
    i2_color = "green" if i2 else "red"
    st.markdown(f"IG1 Continuity: :{i1_color}[**{'YES' if i1 else 'NO'}**] | IG2 Continuity: :{i2_color}[**{'YES' if i2 else 'NO'}**]")

    ig_c1, ig_c2 = st.columns(2)
    if ig_c1.button("FIRE IGNITERS", type="primary", use_container_width=True):
        client.send_command({"command": "ignite"})
    if ig_c2.button("Query Continuity", use_container_width=True):
        client.send_command({"command": "get_igniter_continuity", "id": 1})
        client.send_command({"command": "get_igniter_continuity", "id": 2})

    st.divider()

    # --- SV2 - Rocket (FSW SV via umbilical) ---
    st.subheader("SV2 - Rocket (FSW)")
    sv2_open = client.fsw_telemetry.get("sv_open", False) if client.fsw_connected else False
    sv2_color = "green" if sv2_open else "red"
    sv2_label = "OPEN" if sv2_open else "CLOSED"
    st.markdown(f"State (from FSW telemetry): :{sv2_color}[**{sv2_label}**]")

    sv2_c1, sv2_c2 = st.columns(2)
    if sv2_c1.button("OPEN SV2-Rocket", type="primary", use_container_width=True):
        client.send_command({"command": "fsw_open_sv"})
    if sv2_c2.button("CLOSE SV2-Rocket", use_container_width=True):
        client.send_command({"command": "fsw_close_sv"})

    st.caption("Timed Actuation")
    sv2_tc1, sv2_tc2 = st.columns([2, 1])
    sv2_duration = sv2_tc1.number_input("Duration (s)", min_value=0.1, value=1.0, step=0.1, key="sv2_dur")
    if sv2_tc2.button("Timed Pulse", use_container_width=True):
        client.run_sv2_timed_actuation(sv2_duration)

    st.divider()

    # --- Launch ---
    st.subheader("Launch")
    st.caption("Fires igniters and sends FSW Launch command simultaneously.")
    if st.button("LAUNCH", type="primary", use_container_width=True):
        client.run_launch()

    st.divider()

    # --- Vent Ignite Launch Sequences ---
    st.subheader("Vent Ignite Launch")
    st.caption("Open SV2 + ignite, wait 2s, close SV2, wait delay, open MAV 7.88s, close MAV.")
    vil_c1, vil_c2 = st.columns(2)
    if vil_c1.button("2 SEC LAUNCH", type="primary", use_container_width=True):
        client.run_vent_ignite_launch_2s()
    if vil_c2.button("1 SEC LAUNCH", type="primary", use_container_width=True):
        client.run_vent_ignite_launch_1s()


# --- RIGHT COLUMN: Sensors ---
with col_right:
    st.subheader("Sensor Data (Fill Station ADC)")
    if client.latest_adc:
        adc1 = client.latest_adc.get("adc1", [])
        adc2 = client.latest_adc.get("adc2", [])

        rows = []
        # PT1: ADC1 Ch0
        if len(adc1) > 0:
            ch = adc1[0]
            scaled = ch.get("scaled")
            rows.append({"Sensor": "PT1 (0-1500 PSI)", "Raw": ch["raw"], "Voltage": f"{ch['voltage']:.3f}", "Scaled": f"{scaled:.2f}" if scaled is not None else "N/A"})
        # PT2: ADC1 Ch1
        if len(adc1) > 1:
            ch = adc1[1]
            scaled = ch.get("scaled")
            rows.append({"Sensor": "PT2 (0-1000 PSI)", "Raw": ch["raw"], "Voltage": f"{ch['voltage']:.3f}", "Scaled": f"{scaled:.2f}" if scaled is not None else "N/A"})
        # Load Cell: ADC2 Ch1
        if len(adc2) > 1:
            ch = adc2[1]
            scaled = ch.get("scaled")
            rows.append({"Sensor": "Load Cell", "Raw": ch["raw"], "Voltage": f"{ch['voltage']:.3f}", "Scaled": f"{scaled:.2f}" if scaled is not None else "N/A"})

        if rows:
            st.dataframe(pd.DataFrame(rows), hide_index=True, use_container_width=True)
    else:
        st.info("Waiting for ADC data...")

    st.divider()

    # --- FSW Telemetry Sensors ---
    st.subheader("FSW Sensors (Telemetry)")
    if client.fsw_connected and client.fsw_telemetry:
        t = client.fsw_telemetry
        fsw_rows = [
            {"Sensor": "PT3", "Value": f"{t.get('pt3', 0):.2f}"},
            {"Sensor": "PT4", "Value": f"{t.get('pt4', 0):.2f}"},
            {"Sensor": "RTD", "Value": f"{t.get('rtd', 0):.2f}"},
        ]
        st.dataframe(pd.DataFrame(fsw_rows), hide_index=True, use_container_width=True)
    else:
        st.info("Waiting for FSW telemetry...")


# ==========================================
# BOTTOM SECTION: FSW State + All Umbilical Commands
# ==========================================
st.divider()
st.subheader("Flight Software (Umbilical)")

fsw_status = "Connected" if client.fsw_connected else "Disconnected"
fsw_color = "green" if client.fsw_connected else "red"
st.markdown(f"**Umbilical:** :{fsw_color}[{fsw_status}] | **Flight Mode:** {client.fsw_flight_mode}")

if client.fsw_connected and client.fsw_telemetry:
    t = client.fsw_telemetry

    fc1, fc2, fc3, fc4 = st.columns(4)

    with fc1:
        st.metric("Altitude", f"{t.get('altitude', 0):.1f} m")
        st.metric("Pressure", f"{t.get('pressure', 0):.1f} Pa")
        st.metric("Temperature", f"{t.get('temp', 0):.1f} C")

    with fc2:
        st.metric("Latitude", f"{t.get('latitude', 0):.6f}")
        st.metric("Longitude", f"{t.get('longitude', 0):.6f}")
        st.metric("Satellites", f"{t.get('num_satellites', 0)}")

    with fc3:
        imu_data = [
            {"Axis": "X", "Accel (m/s2)": f"{t.get('accel_x', 0):.2f}", "Gyro (deg/s)": f"{t.get('gyro_x', 0):.2f}", "Mag (uT)": f"{t.get('mag_x', 0):.2f}"},
            {"Axis": "Y", "Accel (m/s2)": f"{t.get('accel_y', 0):.2f}", "Gyro (deg/s)": f"{t.get('gyro_y', 0):.2f}", "Mag (uT)": f"{t.get('mag_y', 0):.2f}"},
            {"Axis": "Z", "Accel (m/s2)": f"{t.get('accel_z', 0):.2f}", "Gyro (deg/s)": f"{t.get('gyro_z', 0):.2f}", "Mag (uT)": f"{t.get('mag_z', 0):.2f}"},
        ]
        st.caption("IMU")
        st.dataframe(pd.DataFrame(imu_data), hide_index=True, use_container_width=True)

    with fc4:
        # Valve states from FSW
        sv_open = t.get("sv_open", False)
        mav_open = t.get("mav_open", False)
        sv_c = "green" if sv_open else "red"
        mav_c = "green" if mav_open else "red"
        st.markdown(f"**SV2-Rocket:** :{sv_c}[{'OPEN' if sv_open else 'CLOSED'}]")
        st.markdown(f"**FSW MAV:** :{mav_c}[{'OPEN' if mav_open else 'CLOSED'}]")

# --- All FSW Umbilical Commands ---
st.caption("FSW Umbilical Commands")
row1 = st.columns(8)
if row1[0].button("FSW Launch", use_container_width=True):
    client.send_command({"command": "fsw_launch"})
if row1[1].button("Open MAV", use_container_width=True):
    client.send_command({"command": "fsw_open_mav"})
if row1[2].button("Close MAV", use_container_width=True):
    client.send_command({"command": "fsw_close_mav"})
if row1[3].button("Open SV", use_container_width=True):
    client.send_command({"command": "fsw_open_sv"})
if row1[4].button("Close SV", use_container_width=True):
    client.send_command({"command": "fsw_close_sv"})
if row1[5].button("FSW Safe", use_container_width=True):
    client.send_command({"command": "fsw_safe"})
if row1[6].button("Reset FRAM", use_container_width=True):
    client.send_command({"command": "fsw_reset_fram"})
if row1[7].button("Reset Card", use_container_width=True):
    client.send_command({"command": "fsw_reset_card"})

row2 = st.columns(8)
if row2[0].button("FSW Reboot", use_container_width=True):
    client.send_command({"command": "fsw_reboot"})
if row2[1].button("Dump Flash", use_container_width=True):
    client.send_command({"command": "fsw_dump_flash"})
if row2[2].button("Wipe Flash", use_container_width=True):
    client.send_command({"command": "fsw_wipe_flash"})
if row2[3].button("Flash Info", use_container_width=True):
    client.send_command({"command": "fsw_flash_info"})
if row2[4].button("Payload N1", use_container_width=True):
    client.send_command({"command": "fsw_payload_n1"})
if row2[5].button("Payload N2", use_container_width=True):
    client.send_command({"command": "fsw_payload_n2"})
if row2[6].button("Payload N3", use_container_width=True):
    client.send_command({"command": "fsw_payload_n3"})
if row2[7].button("Payload N4", use_container_width=True):
    client.send_command({"command": "fsw_payload_n4"})

st.rerun()
