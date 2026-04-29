# BLiMS Reference (Break Line Manipulation System)

> Living document — update whenever the BLiMS control logic, state machine, or hardware interfacing changes.

The BLiMS subsystem is an autonomous, steerable parachute payload. It controls a parafoil by driving two steering lines via a brushless motor/servo setup, utilizing a PI (Proportional-Integral) controller and a GPS-guided state machine to navigate to a target landing coordinate.

---

## 1. Hardware Interface

The motor is controlled via standard RC PWM output:
* **Frame Rate:** 50 Hz
* **Control Range:** `0.0` to `1.0` (Mapped to pulse width via Embassy PWM)
* **Constants:**
  * `0.3` = Maximum Left Turn (`MOTOR_MIN`)
  * `0.5` = Neutral Flight (`NEUTRAL_POS`)
  * `0.7` = Maximum Right Turn (`MOTOR_MAX`)

The system requires an `ENABLE` pin to be pulled high to activate the motor driver (e.g., ODrive).

---

## 2. Flight State Machine (Phases)

The BLiMS controller determines its flight phase strictly based on **Altitude (feet AGL)** and **Distance to Target (feet)**. 

| Altitude Band | Phase | Description | Desired Heading |
|---|---|---|---|
| **> 1000 ft** | `Track` | Distance to target > 400 ft. Navigates straight toward the target. | Bearing to target |
| **> 1000 ft** | `Loiter` | Distance to target < 400 ft. Executes a timed holding pattern to burn altitude. | N/A (Timed turns) |
| **600 – 1000 ft** | `Downwind` | Entering the landing pattern. Flies *with* the wind. | Wind direction + 180° |
| **300 – 600 ft** | `Base` | Flies perpendicular to the wind to set up for final approach. | Crosswind (shortest turn) |
| **100 – 300 ft** | `Final` | Final approach. Flies directly *into* the wind to minimize ground speed at touchdown. | Wind direction |
| **< 100 ft** | `Neutral` | Hands-off touchdown phase. Motor parks at 0.5. | N/A |

*(Note: If the GPS loses lock, the system defaults to the `Held` phase, parking the motor at neutral until a fix is regained).*

---

## 3. Control Loops

### PI Controller
For the `Track`, `Downwind`, `Base`, and `Final` phases, BLiMS uses a PI controller to steer the parachute.
* **Input:** Heading Error (Difference between Desired Heading and GPS Heading of Motion).
* **Output:** Motor Position (Added to the `0.5` neutral baseline).
* **Gains:** `KP = 0.009`, `KI = 0.001`
* **Anti-Windup:** The integral accumulator is clamped to `±10.0` and reset to `0.0` upon any phase change.

### Loiter Pattern
When the payload is high above the target (>1000 ft) but horizontally close (<400 ft), it enters the `Loiter` phase to safely burn altitude. It does not use the PI controller here. Instead, it executes a rigid time-based pattern:
1. **Turn Right** (6 seconds, motor at `0.65`)
2. **Pause Neutral** (2.5 seconds, motor at `0.5`)
3. **Turn Left** (6 seconds, motor at `0.35`)
4. **Pause Neutral** (2.5 seconds, motor at `0.5`)
*(Repeats until altitude drops below 1000 ft)*

---

## 4. Wind Profiling

BLiMS supports a multi-layer wind profile to improve its landing pattern accuracy. 
* The system accepts up to `16` wind layers (altitude in meters, direction from in degrees).
* When calculating the desired heading for `Downwind`, `Base`, or `Final`, the controller linearly interpolates the wind direction at its current altitude.
* If no profile is loaded, it falls back to a single scalar surface wind direction.
