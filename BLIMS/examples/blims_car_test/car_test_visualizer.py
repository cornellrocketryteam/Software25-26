"""
car_test_visualizer.py — BLiMS real-time visualizer (pyqtgraph)
===============================================================
Reads the 12-field CSV stream from car_test.rs (via serial_splitter.py or
directly from the Pico's USB serial port) and displays three live panels:

  1. MAP      – GPS trail, current position dot, target marker,
                heading arrow (blue), bearing-to-target arrow (green)
  2. PI CTRL  – P term (orange) and I term (purple) over a rolling window
  3. ALT/PHASE– Altitude over time, coloured by active BLiMS phase

CSV format (12 fields):
    lat, lon, target_lat, target_lon, heading, bearing,
    motor_pos, timestamp_ms, P, I, phase, altitude

Install
-------
    pip install pyqtgraph PyQt6 pyserial numpy

Usage
-----
    # Recommended: pass the virtual port from serial_splitter.py
    python car_test_visualizer.py /dev/ttys005

    # Or read directly from the Pico (no splitter — no simultaneous terminal)
    python car_test_visualizer.py /dev/cu.usbmodem1101

    # Auto-detect first /dev/cu.usbmodem* if no argument given
    python car_test_visualizer.py
"""

import glob
import sys
import math
import collections
import threading
import queue

import numpy as np
import serial
import pyqtgraph as pg
from pyqtgraph.Qt import QtCore, QtWidgets

# ── CONFIGURATION ─────────────────────────────────────────────────────────────

BAUD_RATE      = 115200
WINDOW_SECONDS = 60
MAX_RATE_HZ    = 20
MAX_LEN        = WINDOW_SECONDS * MAX_RATE_HZ
REFRESH_MS     = 50           # Qt timer interval — 20 Hz redraws

# ── CSV COLUMN INDICES ────────────────────────────────────────────────────────

COL_LAT, COL_LON               = 0, 1
COL_TARGET_LAT, COL_TARGET_LON = 2, 3
COL_HEADING, COL_BEARING       = 4, 5
COL_MOTOR                      = 6
COL_TIMESTAMP_MS               = 7
COL_P, COL_I                   = 8, 9
COL_PHASE                      = 10
COL_ALTITUDE                   = 11
EXPECTED_FIELDS                = 12   # MVP: no loiter_step field

# ── PHASE METADATA ────────────────────────────────────────────────────────────

PHASE_NAMES = {
    0: 'HELD',
    1: 'INITIAL_HOLD',
    2: 'UPWIND',
    3: 'DOWNWIND',
    4: 'NEUTRAL',
}
PHASE_COLORS_RGB = {
    0: (136, 136, 136),  # HELD         – grey
    1: (255, 235,  59),  # INITIAL_HOLD – yellow
    2: ( 33, 150, 243),  # UPWIND       – blue
    3: ( 76, 175,  80),  # DOWNWIND     – green
    4: (244,  67,  54),  # NEUTRAL      – red
}

# MVP altitude thresholds — must match blims_constants.rs
ALT_UPWIND_FT  = 1000.0   # Upwind → Downwind crossover
ALT_NEUTRAL_FT =  200.0   # Downwind → Neutral (hands off)

def phase_color(phase_id):
    return PHASE_COLORS_RGB.get(phase_id, (255, 255, 255))

# ── HELPERS ───────────────────────────────────────────────────────────────────

def wrap180(a):
    a %= 360.0
    if a >  180.0: a -= 360.0
    if a < -180.0: a += 360.0
    return a

def arrow_tip(lon, lat, angle_deg, length=0.0008):
    """Return (tip_lon, tip_lat) for an arrow originating at (lon, lat)."""
    r = math.radians(angle_deg)
    return lon + length * math.sin(r), lat + length * math.cos(r)

def find_pico_port():
    """Return first /dev/cu.usbmodem* port found, or None."""
    candidates = glob.glob('/dev/cu.usbmodem*')
    return candidates[0] if candidates else None

# ── SERIAL READER THREAD ──────────────────────────────────────────────────────

class SerialReader(threading.Thread):
    def __init__(self, port, baud, row_queue):
        super().__init__(daemon=True)
        self.port      = port
        self.baud      = baud
        self.q         = row_queue
        self.connected = False
        self.error     = None

    def run(self):
        try:
            ser = serial.Serial(self.port, self.baud, timeout=1)
            self.connected = True
        except serial.SerialException as e:
            self.error = str(e)
            return

        while True:
            try:
                line = ser.readline().decode(errors='replace').strip()
                if not line or line.startswith('#'):
                    continue
                parts = line.split(',')
                if len(parts) != EXPECTED_FIELDS:
                    continue
                self.q.put((
                    float(parts[COL_LAT]),
                    float(parts[COL_LON]),
                    float(parts[COL_TARGET_LAT]),
                    float(parts[COL_TARGET_LON]),
                    float(parts[COL_HEADING]),
                    float(parts[COL_BEARING]),
                    float(parts[COL_MOTOR]),
                    float(parts[COL_TIMESTAMP_MS]),
                    float(parts[COL_P]),
                    float(parts[COL_I]),
                    int(parts[COL_PHASE]),
                    float(parts[COL_ALTITUDE]),
                ))
            except Exception:
                continue

# ── MAIN WINDOW ───────────────────────────────────────────────────────────────

class BlimsVisualizer(QtWidgets.QMainWindow):
    def __init__(self, serial_port):
        super().__init__()
        self.setWindowTitle('BLiMS MVP Visualizer')
        self.resize(1050, 900)

        # ── Data buffers ─────────────────────────────────────────────────────
        N = MAX_LEN
        self.lats      = collections.deque(maxlen=N)
        self.lons      = collections.deque(maxlen=N)
        self.times     = collections.deque(maxlen=N)
        self.p_terms   = collections.deque(maxlen=N)
        self.i_terms   = collections.deque(maxlen=N)
        self.altitudes = collections.deque(maxlen=N)
        self.phases    = collections.deque(maxlen=N)
        self.latest    = None

        # ── Serial ───────────────────────────────────────────────────────────
        self.row_queue = queue.Queue()
        self.reader = SerialReader(serial_port, BAUD_RATE, self.row_queue)
        self.reader.start()

        # ── Theme ────────────────────────────────────────────────────────────
        pg.setConfigOption('background', '#1e1e2e')
        pg.setConfigOption('foreground', '#cdd6f4')

        # ── Central widget ───────────────────────────────────────────────────
        central = QtWidgets.QWidget()
        self.setCentralWidget(central)
        root = QtWidgets.QVBoxLayout(central)
        root.setSpacing(4)
        root.setContentsMargins(6, 6, 6, 6)

        # Port label
        port_label = QtWidgets.QLabel(f'Port: {serial_port}')
        port_label.setStyleSheet(
            'color:#a6adc8; background:#181825; padding:2px 10px;'
            'border-radius:4px; font-family:monospace; font-size:10px;'
        )
        root.addWidget(port_label)

        # Status bar
        self.status = QtWidgets.QLabel('Waiting for serial data…')
        self.status.setStyleSheet(
            'color:#cdd6f4; background:#313244; padding:4px 10px;'
            'border-radius:4px; font-family:monospace; font-size:11px;'
        )
        root.addWidget(self.status)

        # Graphics area
        self.glw = pg.GraphicsLayoutWidget()
        root.addWidget(self.glw)

        self._build_map_panel()
        self.glw.nextRow()
        self._build_pi_panel()
        self.glw.nextRow()
        self._build_alt_panel()

        # ── Refresh timer ────────────────────────────────────────────────────
        self.timer = QtCore.QTimer()
        self.timer.timeout.connect(self._refresh)
        self.timer.start(REFRESH_MS)

    # ─────────────────────────────────────────────────────────────────────────
    # Panel construction
    # ─────────────────────────────────────────────────────────────────────────

    def _style(self, plot, title, xlabel, ylabel):
        plot.setTitle(title, color='#cdd6f4', size='10pt')
        plot.setLabel('bottom', xlabel, color='#a6adc8', size='9pt')
        plot.setLabel('left',   ylabel, color='#a6adc8', size='9pt')
        plot.showGrid(x=True, y=True, alpha=0.18)
        for axis in ('bottom', 'left'):
            plot.getAxis(axis).setPen(pg.mkPen('#45475a'))
            plot.getAxis(axis).setTextPen(pg.mkPen('#a6adc8'))
        return plot

    def _build_map_panel(self):
        p = self.glw.addPlot(row=0, col=0)
        self._style(p, 'GPS Map', 'Longitude', 'Latitude')
        p.setAspectLocked(True)
        self.map_plot = p

        self.trail_curve = p.plot(pen=pg.mkPen('#45475a', width=1.5))

        self.pos_dot = pg.ScatterPlotItem(
            size=12, pen=pg.mkPen('#cdd6f4', width=1),
            brush=pg.mkBrush(*PHASE_COLORS_RGB[2])   # start colour: UPWIND blue
        )
        p.addItem(self.pos_dot)

        self.target_dot = pg.ScatterPlotItem(
            size=16, symbol='t',
            pen=pg.mkPen('#cdd6f4', width=1),
            brush=pg.mkBrush(244, 67, 54)
        )
        p.addItem(self.target_dot)

        # Heading line + arrowhead (blue)
        self.heading_line = p.plot(pen=pg.mkPen('#2196F3', width=2.5))
        self.heading_head = pg.ArrowItem(
            angle=0, tipAngle=35, headLen=15, tailLen=0,
            brush=pg.mkBrush('#2196F3'), pen=pg.mkPen('#2196F3', width=0)
        )
        p.addItem(self.heading_head)

        # Bearing line + arrowhead (green)
        self.bearing_line = p.plot(pen=pg.mkPen('#4CAF50', width=2.5))
        self.bearing_head = pg.ArrowItem(
            angle=0, tipAngle=35, headLen=15, tailLen=0,
            brush=pg.mkBrush('#4CAF50'), pen=pg.mkPen('#4CAF50', width=0)
        )
        p.addItem(self.bearing_head)

        for text, color, y in [('● Heading', '#2196F3', 1.0),
                                ('● Bearing', '#4CAF50', 0.93),
                                ('▲ Target',  '#F44336', 0.86)]:
            ti = pg.TextItem(text, color=color, anchor=(1, 0))
            ti.setParentItem(p.getViewBox())
            ti.setPos(1.0, y)

    def _build_pi_panel(self):
        p = self.glw.addPlot(row=1, col=0)
        self._style(p, 'PI Controller  (inches)', 'Time (s)', 'Brakeline differential (in)')
        # Motor authority is ±9 in; P/I terms sum to that range
        p.setYRange(-10, 10, padding=0.05)
        self.pi_plot = p

        self.p_curve = p.plot(pen=pg.mkPen('#FAB387', width=2),
                              name='P term')
        self.i_curve = p.plot(pen=pg.mkPen('#CBA6F7', width=2),
                              name='I term')
        self.m_curve = p.plot(
            pen=pg.mkPen('#A6E3A1', width=1.5,
                         style=QtCore.Qt.PenStyle.DashLine),
            name='Motor (P+I)'
        )
        # Neutral reference line at 0 in
        p.addItem(pg.InfiniteLine(
            pos=0, angle=0,
            pen=pg.mkPen('#585b70', width=1,
                         style=QtCore.Qt.PenStyle.DashLine)
        ))

        for text, color, y in [('— P term',    '#FAB387', 1.0),
                                ('— I term',    '#CBA6F7', 0.90),
                                ('-- Motor(in)','#A6E3A1', 0.80)]:
            ti = pg.TextItem(text, color=color, anchor=(1, 0))
            ti.setParentItem(p.getViewBox())
            ti.setPos(1.0, y)

    def _build_alt_panel(self):
        p = self.glw.addPlot(row=2, col=0)
        self._style(p, 'Altitude & Phase', 'Time (s)', 'Altitude (ft)')
        self.alt_plot = p

        self.phase_scatter = {}
        for pid, rgb in PHASE_COLORS_RGB.items():
            s = pg.ScatterPlotItem(size=4, pen=None, brush=pg.mkBrush(*rgb))
            p.addItem(s)
            self.phase_scatter[pid] = s

        # MVP altitude threshold reference lines
        for alt, label, color in [
            (ALT_UPWIND_FT,  'Upwind→Downwind', '#2196F3'),
            (ALT_NEUTRAL_FT, 'Neutral',          '#F44336'),
        ]:
            line = pg.InfiniteLine(
                pos=alt, angle=0,
                pen=pg.mkPen(color, width=1,
                             style=QtCore.Qt.PenStyle.DotLine),
                label=f'  {label}',
                labelOpts={'color': color, 'position': 0.01,
                           'fill': (30, 30, 46, 180)}
            )
            p.addItem(line)

        legend_y = 1.0
        for pid, name in PHASE_NAMES.items():
            rgb = PHASE_COLORS_RGB[pid]
            color = '#{:02x}{:02x}{:02x}'.format(*rgb)
            ti = pg.TextItem(f'● {name}', color=color, anchor=(1, 0))
            ti.setParentItem(p.getViewBox())
            ti.setPos(1.0, legend_y)
            legend_y -= 0.12

    # ─────────────────────────────────────────────────────────────────────────
    # Per-frame update
    # ─────────────────────────────────────────────────────────────────────────

    def _refresh(self):
        if not self.reader.connected and self.reader.error:
            self.status.setText(f'❌  Serial error: {self.reader.error}')
            return

        new_rows = 0
        while True:
            try:
                row = self.row_queue.get_nowait()
            except queue.Empty:
                break
            (lat, lon, tgt_lat, tgt_lon, heading, bearing,
             motor, ts_ms, p, i, phase, alt) = row
            self.lats.append(lat)
            self.lons.append(lon)
            self.times.append(ts_ms / 1000.0)
            self.p_terms.append(p)
            self.i_terms.append(i)
            self.altitudes.append(alt)
            self.phases.append(phase)
            self.latest = row
            new_rows += 1

        if not new_rows or self.latest is None:
            return

        (lat, lon, tgt_lat, tgt_lon, heading, bearing,
         motor, ts_ms, p, i, phase, alt) = self.latest

        t0      = self.times[0]
        rel     = np.array(self.times) - t0
        lats_a  = np.array(self.lats)
        lons_a  = np.array(self.lons)
        p_a     = np.array(self.p_terms)
        i_a     = np.array(self.i_terms)
        alt_a   = np.array(self.altitudes)
        phase_a = np.array(self.phases)

        err        = wrap180(bearing - heading)
        pname      = PHASE_NAMES.get(phase, '???')

        self.status.setText(
            f'  Lat {lat:.6f}   Lon {lon:.6f}   '
            f'Alt {alt:.0f} ft   '
            f'Head {heading:.1f}°   Bearing {bearing:.1f}°   '
            f'Err {err:+.1f}°   Motor {motor:+.3f} in   '
            f'Phase: {pname}'
        )

        # ── MAP ───────────────────────────────────────────────────────────────
        self.trail_curve.setData(lons_a, lats_a)
        self.pos_dot.setData([lon], [lat])
        self.pos_dot.setBrush(pg.mkBrush(*phase_color(phase)))
        self.target_dot.setData([tgt_lon], [tgt_lat])

        tip_lon_h, tip_lat_h = arrow_tip(lon, lat, heading)
        self.heading_line.setData([lon, tip_lon_h], [lat, tip_lat_h])
        self.heading_head.setPos(tip_lon_h, tip_lat_h)
        self.heading_head.setStyle(angle=-(heading - 90))

        tip_lon_b, tip_lat_b = arrow_tip(lon, lat, bearing)
        self.bearing_line.setData([lon, tip_lon_b], [lat, tip_lat_b])
        self.bearing_head.setPos(tip_lon_b, tip_lat_b)
        self.bearing_head.setStyle(angle=-(bearing - 90))

        margin = 0.002
        self.map_plot.setXRange(
            min(lon, tgt_lon) - margin, max(lon, tgt_lon) + margin, padding=0)
        self.map_plot.setYRange(
            min(lat, tgt_lat) - margin, max(lat, tgt_lat) + margin, padding=0)

        # ── PI ────────────────────────────────────────────────────────────────
        self.p_curve.setData(rel, p_a)
        self.i_curve.setData(rel, i_a)
        self.m_curve.setData(rel, p_a + i_a)   # total motor command in inches
        t_now = rel[-1]
        self.pi_plot.setXRange(t_now - WINDOW_SECONDS, t_now, padding=0)

        # ── ALTITUDE / PHASE ──────────────────────────────────────────────────
        for pid, scatter in self.phase_scatter.items():
            mask = phase_a == pid
            if mask.any():
                scatter.setData(rel[mask], alt_a[mask])
            else:
                scatter.setData([], [])

        self.alt_plot.setXRange(t_now - WINDOW_SECONDS, t_now, padding=0)
        if alt_a.size:
            self.alt_plot.setYRange(
                max(0, float(alt_a.min()) - 50),
                float(alt_a.max()) + 100,
                padding=0
            )

# ── ENTRY POINT ───────────────────────────────────────────────────────────────

if __name__ == '__main__':
    if len(sys.argv) > 1:
        port = sys.argv[1]
    else:
        port = find_pico_port()
        if port is None:
            print("ERROR: No /dev/cu.usbmodem* port found and none given as argument.")
            print("Usage: python car_test_visualizer.py <port>")
            sys.exit(1)
        print(f"[visualizer] Auto-detected port: {port}")

    app = QtWidgets.QApplication(sys.argv)
    app.setStyle('Fusion')
    win = BlimsVisualizer(port)
    win.show()
    sys.exit(app.exec())