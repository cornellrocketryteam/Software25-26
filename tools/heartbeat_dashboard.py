#!/usr/bin/env python3
"""
FSW Heartbeat Dashboard — testing tool for the heartbeat-based umbilical link.

What it does:
  - Auto-detects (or accepts) the FSW USB-CDC serial port.
  - Sends `<H>` to the FSW once per second so the FSW's `is_connected()` stays True.
  - Reads incoming text lines:
      * `$TELEM,...` lines are parsed and shown in a live table.
      * Everything else is appended to a scrolling FSW-log pane.
  - Lets you send commands by typing single keys (L, M, m, S, s, V, ...).

Usage:
  python3 heartbeat_dashboard.py                       # auto-detect port
  python3 heartbeat_dashboard.py /dev/cu.usbmodem1234  # specify port

Quit with `q` or Ctrl-C.
"""

import sys
import glob
import time
import curses
import threading
from collections import deque

import serial

BAUD = 115200
HEARTBEAT_INTERVAL_S = 1.0
LOG_BUFFER_LINES = 500

TELEM_FIELDS = [
    "flight_mode", "pressure", "temp", "altitude",
    "latitude", "longitude", "num_satellites", "timestamp",
    "mag_x", "mag_y", "mag_z",
    "accel_x", "accel_y", "accel_z",
    "gyro_x", "gyro_y", "gyro_z",
    "pt3", "pt4", "rtd",
    "sv_open", "mav_open",
    "ssa_drogue_deployed", "ssa_main_deployed",
    "cmd_n1", "cmd_n2", "cmd_n3", "cmd_n4",
    "cmd_a1", "cmd_a2", "cmd_a3",
    "airbrake_state", "predicted_apogee",
    "h_acc", "v_acc",
    "vel_n", "vel_e", "vel_d", "g_speed",
    "s_acc", "head_acc", "fix_type", "head_mot",
    "blims_motor_position", "blims_phase_id",
    "blims_pid_p", "blims_pid_i", "blims_bearing",
    "blims_loiter_step", "blims_heading_des",
    "blims_heading_error", "blims_error_integral",
    "blims_dist_to_target_m",
    "blims_target_lat", "blims_target_lon", "blims_wind_from_deg",
]

MODE_NAMES = {
    0: "Startup", 1: "Standby", 2: "Ascent", 3: "Coast",
    4: "DrogueDeployed", 5: "MainDeployed", 6: "Fault",
}

COMMANDS = {
    "L": ("<L>", "Launch"),
    "M": ("<M>", "Open MAV"),
    "m": ("<m>", "Close MAV"),
    "S": ("<S>", "Open SV"),
    "s": ("<s>", "Close SV"),
    "V": ("<V>", "Safe"),
    "D": ("<D>", "Reset Card"),
    "F": ("<F>", "Reset FRAM"),
    "f": ("<f>", "Dump FRAM"),
    "R": ("<R>", "Reboot"),
    "G": ("<G>", "Dump Flash"),
    "W": ("<W>", "Wipe Flash"),
    "I": ("<I>", "Flash Info"),
    "X": ("<X>", "Fault Mode"),
    "1": ("<1>", "Payload N1"),
    "2": ("<2>", "Payload N2"),
    "3": ("<3>", "Payload N3"),
    "4": ("<4>", "Payload N4"),
}


def find_port():
    for pat in ("/dev/cu.usbmodem*", "/dev/ttyACM*"):
        ports = glob.glob(pat)
        if ports:
            return ports[0]
    return None


def parse_telemetry(csv_str):
    parts = [p.strip() for p in csv_str.split(",")]
    if len(parts) < len(TELEM_FIELDS):
        return None
    out = {}
    for i, name in enumerate(TELEM_FIELDS):
        v = parts[i]
        try:
            out[name] = int(v) if "." not in v and "e" not in v.lower() else float(v)
        except ValueError:
            try:
                out[name] = float(v)
            except ValueError:
                return None
    return out


class State:
    def __init__(self):
        self.lock = threading.Lock()
        self.telem = None
        self.last_telem_t = 0.0
        self.telem_count = 0
        self.heartbeat_count = 0
        self.last_heartbeat_t = 0.0
        self.logs = deque(maxlen=LOG_BUFFER_LINES)
        self.write_error = None
        self.read_error = None
        self.stop = False

    def add_log(self, line):
        with self.lock:
            self.logs.append((time.time(), line))


def reader_thread(ser, state):
    buf = b""
    while not state.stop:
        try:
            chunk = ser.read(256)
            if not chunk:
                continue
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                text = line.decode("utf-8", errors="replace").strip()
                if not text:
                    continue
                if text.startswith("$TELEM,"):
                    telem = parse_telemetry(text[7:])
                    if telem is not None:
                        with state.lock:
                            state.telem = telem
                            state.last_telem_t = time.time()
                            state.telem_count += 1
                    else:
                        state.add_log(f"[parse-fail] {text[:80]}")
                else:
                    state.add_log(text)
        except Exception as e:
            state.read_error = str(e)
            return


def heartbeat_thread(ser, state):
    while not state.stop:
        try:
            ser.write(b"<H>")
            with state.lock:
                state.heartbeat_count += 1
                state.last_heartbeat_t = time.time()
        except Exception as e:
            state.write_error = str(e)
            return
        time.sleep(HEARTBEAT_INTERVAL_S)


def safe_addstr(win, y, x, s, attr=0):
    h, w = win.getmaxyx()
    if y < 0 or y >= h or x >= w:
        return
    s = s[: max(0, w - x - 1)]
    try:
        win.addstr(y, x, s, attr)
    except curses.error:
        pass


def fmt_age(t_now, t):
    if t == 0:
        return "never"
    d = t_now - t
    if d < 1.0:
        return f"{int(d * 1000)}ms"
    return f"{d:.1f}s"


def draw(stdscr, state, port_name):
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    now = time.time()

    with state.lock:
        telem = state.telem
        last_telem_t = state.last_telem_t
        telem_count = state.telem_count
        hb_count = state.heartbeat_count
        last_hb_t = state.last_heartbeat_t
        logs = list(state.logs)
        write_err = state.write_error
        read_err = state.read_error

    # Header
    title = f" FSW Heartbeat Dashboard — {port_name} @ {BAUD} "
    safe_addstr(stdscr, 0, 0, title.ljust(w - 1), curses.A_REVERSE)

    # Status line
    telem_age = fmt_age(now, last_telem_t)
    hb_age = fmt_age(now, last_hb_t)
    safe_addstr(stdscr, 1, 0,
                f"HB tx: {hb_count} ({hb_age} ago)   "
                f"TELEM rx: {telem_count} ({telem_age} ago)")
    if write_err:
        safe_addstr(stdscr, 1, 0, f"WRITE ERROR: {write_err}", curses.A_BOLD)
    if read_err:
        safe_addstr(stdscr, 2, 0, f"READ ERROR: {read_err}", curses.A_BOLD)

    # Telemetry table
    row = 3
    safe_addstr(stdscr, row, 0, "─ Telemetry ".ljust(w - 1, "─"))
    row += 1
    if telem is None:
        safe_addstr(stdscr, row, 2, "(no telemetry yet)")
        row += 1
    else:
        mode = MODE_NAMES.get(telem["flight_mode"], f"?{telem['flight_mode']}")
        sv = "OPEN" if telem["sv_open"] else "closed"
        mav = "OPEN" if telem["mav_open"] else "closed"
        lines = [
            f"mode={mode:<14}  alt={telem['altitude']:>8.2f} m   pres={telem['pressure']:>9.1f} Pa   temp={telem['temp']:>6.2f} C",
            f"lat={telem['latitude']:>10.5f}  lon={telem['longitude']:>10.5f}  sats={telem['num_satellites']:<3}  fix={telem['fix_type']}",
            f"accel=({telem['accel_x']:>6.2f},{telem['accel_y']:>6.2f},{telem['accel_z']:>6.2f})  gyro=({telem['gyro_x']:>6.1f},{telem['gyro_y']:>6.1f},{telem['gyro_z']:>6.1f})",
            f"pt3={telem['pt3']:>7.1f}  pt4={telem['pt4']:>7.1f}  rtd={telem['rtd']:>7.1f}   SV={sv:<6}  MAV={mav:<6}",
            f"airbrake={telem['airbrake_state']}  predicted_apogee={telem['predicted_apogee']:.1f} m   drogue={telem['ssa_drogue_deployed']} main={telem['ssa_main_deployed']}",
        ]
        for line in lines:
            safe_addstr(stdscr, row, 2, line)
            row += 1

    # Log pane
    row += 1
    safe_addstr(stdscr, row, 0, "─ FSW log ".ljust(w - 1, "─"))
    row += 1
    log_capacity = h - row - 3
    if log_capacity > 0:
        recent = logs[-log_capacity:]
        for i, (t, msg) in enumerate(recent):
            ts = time.strftime("%H:%M:%S", time.localtime(t))
            safe_addstr(stdscr, row + i, 0, f"{ts}  {msg}")

    # Footer / commands
    foot_row = h - 2
    cmds_help = "  ".join(f"{k}={v[1]}" for k, v in list(COMMANDS.items())[:6])
    safe_addstr(stdscr, foot_row, 0, "─" * (w - 1))
    safe_addstr(stdscr, h - 1, 0, f"Keys: q=quit  ?=cmd help   {cmds_help}", curses.A_DIM)

    stdscr.refresh()


def show_help(stdscr):
    h, w = stdscr.getmaxyx()
    win = curses.newwin(min(len(COMMANDS) + 4, h - 2), min(40, w - 2), 2, 2)
    win.box()
    win.addstr(0, 2, " Commands ")
    for i, (k, (_tok, name)) in enumerate(COMMANDS.items()):
        win.addstr(1 + i, 2, f"  {k}  →  {name}")
    win.addstr(len(COMMANDS) + 2, 2, " Press any key ")
    win.refresh()
    win.getch()


def run(stdscr, ser, port_name):
    curses.curs_set(0)
    stdscr.nodelay(True)
    stdscr.timeout(100)

    state = State()
    threading.Thread(target=reader_thread, args=(ser, state), daemon=True).start()
    threading.Thread(target=heartbeat_thread, args=(ser, state), daemon=True).start()

    while True:
        draw(stdscr, state, port_name)
        try:
            ch = stdscr.getch()
        except KeyboardInterrupt:
            break
        if ch == -1:
            continue
        if ch in (ord("q"), ord("Q")):
            break
        if ch == ord("?"):
            show_help(stdscr)
            continue
        try:
            key = chr(ch)
        except ValueError:
            continue
        if key in COMMANDS:
            tok, name = COMMANDS[key]
            try:
                ser.write(tok.encode())
                state.add_log(f"[cmd] sent {tok} ({name})")
            except Exception as e:
                state.add_log(f"[cmd] write failed: {e}")

    state.stop = True


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else find_port()
    if not port:
        print("No FSW serial port found. Pass one explicitly.", file=sys.stderr)
        sys.exit(1)
    try:
        ser = serial.Serial(port, BAUD, timeout=0.1, write_timeout=1.0)
    except serial.SerialException as e:
        print(f"Failed to open {port}: {e}", file=sys.stderr)
        sys.exit(1)

    try:
        curses.wrapper(run, ser, port)
    finally:
        try:
            ser.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()
