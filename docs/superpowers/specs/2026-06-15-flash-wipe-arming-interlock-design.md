# Flash-Wipe Arming Interlock (Startup → Standby)

**Date:** 2026-06-15
**Subsystem:** `fsw/` (Flight Software)
**Status:** Approved design

## Summary

Add a mandatory precondition to the FSW `Startup → Standby` transition: a flash
wipe command must have **completed successfully** since the board last entered
the Startup window. The permission is consumed on every dropback to Startup and
on every boot, so the operator must perform a fresh, successful wipe before each
arming.

## Motivation

Arming should guarantee the onboard flash data-log region is empty so a flight
never records on top of stale data. Making a successful wipe a hard
precondition for entering Standby enforces this operationally, every time.

## Current Behavior

The Startup → Standby transition lives in the `match` arm at
`fsw/src/flight_loop.rs:687`. Today it requires, simultaneously:

1. `cfc_arm_active` — GPIO 41 (CFC_ARM) high.
2. `umbilical_connected` — USB umbilical live.
3. `altimeter_state == VALID`.

When satisfied, it latches `arming_altitude`, sets `alt_armed = true`, writes a
recovery snapshot, and sets `flight_mode = Standby`.

Standby reverts to Startup when CFC_ARM goes low (`fsw/src/flight_loop.rs:743`).

The flash wipe is triggered by `UmbilicalCommand::WipeFlash`
(`fsw/src/flight_loop.rs:478`), which calls
`FlightState::wipe_flash_storage()` (`fsw/src/state.rs:821`). That method
currently logs success/failure but returns nothing.

## Design Decisions (resolved)

- **Reset timing:** Cleared on dropback (Standby → Startup) **and** on boot.
- **Success definition:** Only a wipe whose underlying `wipe_storage()` returns
  `Ok` counts. Timeout or error does **not** set the flag.
- **Feedback:** On a blocked arming attempt, print a message over the umbilical
  and emit a distinct buzz.
- **Wipe timing within window:** A successful wipe performed at **any** point
  during the current Startup window satisfies the requirement — it need not be
  the immediately preceding command.

## Changes

### 1. `wipe_flash_storage()` reports success — `fsw/src/state.rs:821`

Change the signature from `async fn wipe_flash_storage(&mut self)` to return
`bool`: `true` only on the `Ok(Ok(_))` branch; `false` on `Ok(Err(_))` or the
timeout `Err(_)` branch. Existing log/umbilical messages are preserved.

Update the three callers:

- `fsw/src/flight_loop.rs:480` (`WipeFlash` arm) — consume the result (see #3).
- `fsw/src/flight_loop.rs:530` (`WipeFramReboot`) — reboots immediately; result
  ignored (`let _ = ...`).
- `fsw/src/main.rs:220` (boot-time wipe) — result ignored.

### 2. New `flash_wiped` flag on `FlightLoop` — `fsw/src/flight_loop.rs:21`

Add a private field:

```rust
flash_wiped: bool,
```

Initialized to `false` in `FlightLoop::new()`. Default-false on construction
satisfies the boot-reset requirement with no extra logic.

Meaning: "a successful wipe has occurred since entering the current Startup
window."

### 3. Set the flag in the `WipeFlash` arm — `fsw/src/flight_loop.rs:478-481`

```rust
UmbilicalCommand::WipeFlash => {
    log::warn!("UMBILICAL CMD: Wipe Flash Data");
    if self.flight_state.wipe_flash_storage().await {
        self.flash_wiped = true;
    }
}
```

The flag is set on any successful wipe regardless of mode; its meaning is scoped
by the reset logic (#6), not by the mode at wipe time.

### 4. Gate the transition — `fsw/src/flight_loop.rs:687`

Add `&& self.flash_wiped` to the arming condition:

```rust
if self.flight_state.cfc_arm_active
    && self.flight_state.umbilical_connected
    && self.flash_wiped
{
    if self.flight_state.altimeter_state == SensorState::VALID {
        // ... existing latch arming_altitude / set Standby / snapshot ...
    }
}
```

### 5. Operator feedback (edge-triggered) — `fsw/src/flight_loop.rs:633-638`

Reuse the existing CFC_ARM rising-edge block so feedback fires once per arming
attempt rather than every 20 Hz cycle:

```rust
let cfc_arm_now = self.flight_state.cfc_arm_active;
if cfc_arm_now && !self.cfc_arm_prev {
    log::info!("CFC_ARM detected: arming signal received");
    if self.flash_wiped {
        self.flight_state.buzz(2);            // existing arm ack
    } else {
        crate::umbilical::print_str("Arming blocked: wipe flash first\n");
        self.flight_state.buzz(5);            // distinct reject pattern
    }
}
self.cfc_arm_prev = cfc_arm_now;
```

(Exact reject beat count to be finalized in implementation; must be visually/
audibly distinct from the 2-beep ack and the 3-beep umbilical-disconnect.)

### 6. Clear the flag on dropback — `fsw/src/flight_loop.rs:743-748`

In the Standby → Startup revert:

```rust
if !self.flight_state.cfc_arm_active {
    self.flash_wiped = false;
    self.flight_state.flight_mode = FlightMode::Startup;
    self.flight_state.write_packet_to_fram().await;
    log::info!("CFC_ARM low in Standby; transitioning back to Startup");
    return;
}
```

Combined with default-false on boot, this satisfies "dropback + reboot" reset.

### 7. Documentation

- `FAILSAFES.md`: document the wipe interlock as an arming safety precondition
  (condition, reset semantics, operator feedback).
- `UMBILICAL_REFERENCE.md`: note that a successful `WipeFlash` is required before
  CFC_ARM will arm to Standby.

## Out of Scope (YAGNI)

- **No telemetry packet field.** Exposing `flash_wiped` would ripple across the
  54-field umbilical CSV (`fill-station/src/components/umbilical.rs`), the
  199-byte RFD/RATS binary packet, and the MQTT `TelemetryPayload`/DB schema.
  The operator already receives the wipe-success message and the arming-reject
  feedback over the umbilical. Dashboard arming-readiness display is a possible
  future follow-up, tracked separately.
- No new timer/throttle state — feedback rides the existing CFC_ARM edge.

## Testing

- **Unit/sim:** Using the existing sim overrides (`sim_cfc_arm_override`),
  verify:
  1. CFC_ARM high + umbilical + valid altimeter but `flash_wiped == false` stays
     in Startup and does not latch `arming_altitude`.
  2. After a successful `WipeFlash`, the same conditions transition to Standby.
  3. A failed/timed-out wipe leaves `flash_wiped == false` and does not arm.
  4. Standby → Startup dropback clears `flash_wiped`; re-arming requires a new
     wipe.
- **Build:** `cd fsw && cargo build` clean.
- **Bench:** On hardware, confirm the reject buzz/umbilical message fires on a
  CFC_ARM toggle without a prior wipe, and that arming succeeds after a wipe.
