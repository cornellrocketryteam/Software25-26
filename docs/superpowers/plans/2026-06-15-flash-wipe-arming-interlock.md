# Flash-Wipe Arming Interlock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Require a successful flash wipe before the FSW will transition Startup → Standby, re-required on every dropback to Startup and every boot.

**Architecture:** Add a private `flash_wiped` flag to `FlightLoop`. A successful `WipeFlash` umbilical command sets it; the Startup→Standby arming condition reads it; falling back Standby→Startup (and rebooting) clears it. A blocked arming attempt is surfaced via the existing CFC_ARM rising-edge handler with a distinct buzz + umbilical message.

**Tech Stack:** `no_std` Rust, Embassy framework, RP2350 target. No host test harness exists for this crate — verification is `cargo build` (the compiler enforces the type changes), feature-gated sim builds, and on-bench manual checks.

---

## Verification Notes (read first)

- Primary gate: `cd fsw && cargo build`. The `wipe_flash_storage()` signature change forces the compiler to flag every caller, so a clean build proves all call sites were updated.
- Sim build that exercises the boot-wipe caller: `cd fsw && cargo build --features sim_real_flight`.
- There are **no** `#[test]` unit tests in `fsw/` — do not add a `cargo test` step; it will not run for this target. Behavioral checks are listed as a manual bench checklist in Task 5.

## File Structure

- Modify: `fsw/src/state.rs` — `wipe_flash_storage()` returns `bool`.
- Modify: `fsw/src/flight_loop.rs` — new `flash_wiped` field; set/gate/clear/feedback logic.
- Modify: `fsw/src/main.rs` — update boot-wipe caller for the new return type.
- Modify: `FAILSAFES.md`, `UMBILICAL_REFERENCE.md` — document the interlock.

---

### Task 1: `wipe_flash_storage()` reports success

**Files:**
- Modify: `fsw/src/state.rs:821-840`
- Modify: `fsw/src/flight_loop.rs:530` (WipeFramReboot caller)
- Modify: `fsw/src/main.rs:220` (boot-wipe caller)

- [ ] **Step 1: Change the method to return `bool`**

In `fsw/src/state.rs`, replace the signature and the match arms so the method returns whether the wipe succeeded. Final form:

```rust
    /// Erases all stored CSV data in the flash storage region.
    /// Returns `true` only if the underlying wipe completed successfully.
    pub async fn wipe_flash_storage(&mut self) -> bool {
        log::info!("Wiping QSPI Flash storage...");
        crate::umbilical::print_str("Wiping QSPI Flash... Please wait.\n");
        // Wiping a full 14 MB flash can take several minutes — use a dedicated timeout.
        let wipe_to = Duration::from_millis(constants::FLASH_WIPE_TIMEOUT_MS);
        match with_timeout(wipe_to, self.flash.wipe_storage()).await {
            Ok(Ok(_)) => {
                log::info!("Flash storage wiped successfully.");
                crate::umbilical::print_str("Flash wiped successfully.\n");
                true
            }
            Ok(Err(e)) => {
                log::error!("Failed to wipe flash storage: {:?}", e);
                crate::umbilical::print_str("ERASE FAILED!\n");
                false
            }
            Err(_) => {
                log::error!("Flash wipe TIMEOUT");
                crate::umbilical::print_str("ERASE TIMEOUT!\n");
                false
            }
        }
    }
```

- [ ] **Step 2: Update the WipeFramReboot caller**

In `fsw/src/flight_loop.rs:530`, the result is irrelevant because the board reboots immediately after. Discard it explicitly:

```rust
                UmbilicalCommand::WipeFramReboot => {
                    log::warn!("UMBILICAL CMD: Wipe Flash + FRAM and Reboot");
                    let _ = self.flight_state.wipe_flash_storage().await;
                    self.flight_state.reset_fram().await;
                    cortex_m::peripheral::SCB::sys_reset();
                }
```

- [ ] **Step 3: Update the boot-wipe caller**

In `fsw/src/main.rs:220` (inside the `sim_real_flight` block), discard the result:

```rust
            let _ = flight_loop.flight_state.wipe_flash_storage().await;
```

> Note: the `WipeFlash` arm at `fsw/src/flight_loop.rs:480` is updated in Task 2, not here. Leaving it calling the now-`bool`-returning method without using the value compiles (unused result is a warning at most, and `wipe_flash_storage` is not `#[must_use]`), but Task 2 immediately consumes it.

- [ ] **Step 4: Build**

Run: `cd fsw && cargo build`
Expected: compiles cleanly. Then `cargo build --features sim_real_flight` — also clean (verifies the `main.rs` caller).

- [ ] **Step 5: Commit**

```bash
git add fsw/src/state.rs fsw/src/flight_loop.rs fsw/src/main.rs
git commit -m "fsw: wipe_flash_storage reports success"
```

---

### Task 2: Add `flash_wiped` flag, set it, gate the transition, clear on dropback

**Files:**
- Modify: `fsw/src/flight_loop.rs` — struct field (~line 46), init (~line 188), WipeFlash arm (line 478), arming gate (line 687), dropback (line 743)

This task adds the field together with both its writer and its reader so there are no dead-code warnings in any intermediate state.

- [ ] **Step 1: Add the struct field**

In `fsw/src/flight_loop.rs`, add to the `FlightLoop` struct near the other private bool flags (e.g. right after `vent_signal_sent: bool,` at line 46):

```rust
    /// Set true by a successful WipeFlash command; required to arm Startup→Standby.
    /// Cleared on dropback to Startup and false on boot, so a fresh wipe is
    /// required before each arming.
    flash_wiped: bool,
```

- [ ] **Step 2: Initialize the field**

In `FlightLoop::new()`, near `vent_signal_sent: false,` (line 188), add:

```rust
            flash_wiped: false,
```

- [ ] **Step 3: Set the flag on a successful wipe**

Replace the `WipeFlash` arm at `fsw/src/flight_loop.rs:478-481`:

```rust
                UmbilicalCommand::WipeFlash => {
                    log::warn!("UMBILICAL CMD: Wipe Flash Data");
                    if self.flight_state.wipe_flash_storage().await {
                        self.flash_wiped = true;
                        log::info!("Flash wipe confirmed — arming permitted.");
                    }
                }
```

- [ ] **Step 4: Gate the Startup→Standby transition**

At `fsw/src/flight_loop.rs:687`, add the `flash_wiped` precondition. Replace:

```rust
                if self.flight_state.cfc_arm_active && self.flight_state.umbilical_connected {
```

with:

```rust
                if self.flight_state.cfc_arm_active
                    && self.flight_state.umbilical_connected
                    && self.flash_wiped
                {
```

(The inner `altimeter_state == VALID` check and the latch/snapshot body are unchanged.)

- [ ] **Step 5: Clear the flag on dropback**

In the Standby→Startup revert at `fsw/src/flight_loop.rs:743-748`, set the flag false before transitioning:

```rust
                if !self.flight_state.cfc_arm_active {
                    self.flash_wiped = false;
                    self.flight_state.flight_mode = FlightMode::Startup;
                    self.flight_state.write_packet_to_fram().await;
                    log::info!("CFC_ARM low in Standby; transitioning back to Startup");
                    return;
                }
```

- [ ] **Step 6: Build**

Run: `cd fsw && cargo build`
Expected: compiles cleanly, no dead-code warning for `flash_wiped` (it is both written in Step 3 and read in Step 4).

- [ ] **Step 7: Commit**

```bash
git add fsw/src/flight_loop.rs
git commit -m "fsw: require successful flash wipe to arm Startup->Standby"
```

---

### Task 3: Operator feedback on blocked arming attempt

**Files:**
- Modify: `fsw/src/flight_loop.rs:633-638` (CFC_ARM rising-edge block)

- [ ] **Step 1: Branch the CFC_ARM rising-edge handler**

Replace the existing block at `fsw/src/flight_loop.rs:633-638`:

```rust
        // CFC_ARM rising edge: buzz once when arming signal goes high
        let cfc_arm_now = self.flight_state.cfc_arm_active;
        if cfc_arm_now && !self.cfc_arm_prev {
            log::info!("CFC_ARM detected: arming signal received");
            self.flight_state.buzz(2);
        }
        self.cfc_arm_prev = cfc_arm_now;
```

with:

```rust
        // CFC_ARM rising edge: acknowledge arming, or reject if no wipe was done.
        let cfc_arm_now = self.flight_state.cfc_arm_active;
        if cfc_arm_now && !self.cfc_arm_prev {
            log::info!("CFC_ARM detected: arming signal received");
            if self.flash_wiped {
                self.flight_state.buzz(2); // arm ack
            } else {
                log::warn!("Arming blocked: flash not wiped");
                crate::umbilical::print_str("Arming blocked: wipe flash first\n");
                self.flight_state.buzz(5); // distinct reject pattern
            }
        }
        self.cfc_arm_prev = cfc_arm_now;
```

The 5-beep reject is distinct from the 2-beep ack and the 3-beep umbilical-disconnect already in use.

- [ ] **Step 2: Build**

Run: `cd fsw && cargo build`
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add fsw/src/flight_loop.rs
git commit -m "fsw: buzz + umbilical message when arming blocked by missing wipe"
```

---

### Task 4: Documentation

**Files:**
- Modify: `FAILSAFES.md`
- Modify: `UMBILICAL_REFERENCE.md`

- [ ] **Step 1: Read both docs to find the right sections**

Run: open `FAILSAFES.md` and `UMBILICAL_REFERENCE.md`. Locate the arming/Standby section in `FAILSAFES.md` and the `WipeFlash` / arming command description in `UMBILICAL_REFERENCE.md`.

- [ ] **Step 2: Add the interlock to `FAILSAFES.md`**

Add an entry describing: a successful `WipeFlash` is a precondition for Startup→Standby; the permission is cleared on dropback to Startup and on boot; a blocked attempt produces a 5-beep buzz and the umbilical message `Arming blocked: wipe flash first`. Match the file's existing heading/table style.

- [ ] **Step 3: Note the requirement in `UMBILICAL_REFERENCE.md`**

In the section covering `WipeFlash` / arming, add a sentence: a successful flash wipe must precede CFC_ARM for the board to arm to Standby, and must be repeated for each arming.

- [ ] **Step 4: Commit**

```bash
git add FAILSAFES.md UMBILICAL_REFERENCE.md
git commit -m "docs: document flash-wipe arming interlock"
```

---

### Task 5: Bench verification checklist (manual, on hardware)

No code changes. Confirm on a board with the umbilical connected:

- [ ] Boot the board into Startup. With CFC_ARM high + umbilical connected + altimeter valid but **no** wipe performed, toggling CFC_ARM produces a 5-beep reject and the umbilical prints `Arming blocked: wipe flash first`; mode stays Startup (verify via telemetry `flight_mode`).
- [ ] Send `WipeFlash`; observe `Flash wiped successfully.` over the umbilical.
- [ ] With CFC_ARM high again, the board now transitions to Standby (2-beep ack, telemetry shows mode 1).
- [ ] Drop CFC_ARM low: board returns to Startup. Raise CFC_ARM again **without** re-wiping: arming is rejected again (confirms dropback cleared the flag).
- [ ] Trigger a failed/timed-out wipe path if feasible (e.g. flash not accessible): confirm `flash_wiped` stays false and arming remains blocked.

---

## Self-Review

- **Spec coverage:** wipe returns success (Task 1), flag + set + gate + clear (Task 2), edge-triggered feedback (Task 3), docs in FAILSAFES.md + UMBILICAL_REFERENCE.md (Task 4), sim/bench testing (Task 5). Out-of-scope telemetry field correctly omitted. All spec sections mapped.
- **Placeholder scan:** none — every code step shows full code; the only "read to locate" step (Task 4 Step 1) is inherent to editing prose docs whose exact line numbers aren't fixed here.
- **Type consistency:** `wipe_flash_storage()` returns `bool` (Task 1) and is consumed as a bool in Task 2 Step 3; `flash_wiped: bool` is defined (Task 2 Step 1) and read identically in Tasks 2 and 3.
