# Ground Station UI — Bug Report

Bugs found and fixed during code review session (2026-04-28).
Bug 1 (faked pt3 pressure simulation) is excluded — handled separately.

---

## Bug 2 — INITIATE SAFE PROCEDURE Did Almost Nothing

**File:** `src/components/subcomponents/InitialFillComponent.tsx`

### What the Issue Was
The "INITIATE SAFE PROCEDURE" button, which appears during an active automated fill, only closed the Ball Valve and set `isVentingRef.current = true`. It did not:
- Open SV2 to vent the accumulated pressure
- Transition `fillState` to `'SAFE_PROCEDURE'` (so the UI never changed screens)
- Stop the fill loop (`isFillingRef` was left true)
- Re-enable button interaction so the operator could take manual control

### How It Could Cause an Issue
An operator sees pressure climbing and hits SAFE PROCEDURE expecting the system to vent and enter a controlled safe state. Instead, BV closes (fill pauses), but SV2 never opens so pressure is not released. The fill loop is still technically running. The screen stays on the fill UI — there is no visual confirmation that anything happened. The rocket tank retains full pressure with no vent pathway and no UI feedback.

### The Fix
The button now executes the full safe sequence:
1. Stops the fill loop (`isFillingRef.current = false`, `setFillUIActive(false)`)
2. Closes BV immediately
3. Opens SV2 to vent — **SV2 stays open indefinitely**. There is no auto-close timer. The operator must manually close SV2 using the Solenoid Valve 2 button in the valve grid when they are satisfied with the vent.
4. Re-enables button interaction (`buttonInteractionState = 'ENABLED'`) so the operator has full manual control of all valves
5. Immediately transitions `fillState` to `'SAFE_PROCEDURE'`, which renders `SafeFillComponent` — a confirmation screen indicating the procedure is active

---

## Bug 3 — VENT "ABORT" Did Not Actually Abort

**File:** `src/components/VentButtonComponent.tsx`

### What the Issue Was
When a vent was in progress, the VENT button label changed to "ABORT" but its `onClick` handler still executed `manualVentRef.current = true` — the same action as requesting a new manual vent. There was no code path to stop the current vent. The button was also `disabled` while `ventUIActive` was true, making "ABORT" completely unclickable anyway.

### How It Could Cause an Issue
An operator initiates a vent, then realizes they need to stop it early (e.g., pressure dropped faster than expected, or they vented by mistake). They click ABORT. Nothing happens — the vent runs its full duration with SV2 open the entire time. In a worst case where vent seconds was set high (up to 10s), the system vents far longer than intended with no way to interrupt it from the UI.

### The Fix
The `onClick` now branches on `ventUIActive`:
- **If venting:** closes SV2 immediately via `handleButtonClickRef`, sets `isVentingRef.current = false`, and calls `setVentUIActive(false)` to clear the UI state. The fill loop sees `isVentingRef` cleared on its next 200ms tick and resumes normal operation.
- **If not venting:** triggers a manual vent as before (`manualVentRef.current = true`)

The `disabled` condition was also corrected — ABORT must never be disabled during a vent, so `ventUIActive` was removed from the disabled guard.

---

## Bug 4 — Recovery Page Coordinates Were Never Sent to the Server

**File:** `src/RecoveryPage.tsx`

### What the Issue Was
Two separate problems:

1. **Command never sent:** The "Confirm Coordinates" button called `setTargetLocationCommand(lat, lon)`, which was a function that created and returned a command object — but never called `wsRef.current?.send()`. The return value was silently discarded. Confirming coordinates only updated local display state.

2. **Wrong field names:** The `setTargetLocationCommand` helper used `"latitude"` and `"longitude"` as JSON keys. The fill-station server (`command.rs:128`) deserializes `FswSetBlimsTarget` with fields named `lat` and `lon`. The server would have rejected the message as unparseable even if it had been sent.

### How It Could Cause an Issue
The operator inputs the landing zone coordinates before launch, sees the display update with the confirmed values, and believes BLiMS has been given a target. In reality the rocket's BLiMS system never receives the target. At drogue deploy, BLiMS begins its landing guidance loop with either no target or a stale target from a previous flight, potentially steering the rocket toward the wrong location.

### The Fix
The helper function was removed. The button `onClick` now directly calls:
```ts
wsRef.current?.send(JSON.stringify({ "command": "fsw_set_blims_target", lat, lon }));
```
Field names match the server exactly (`lat`, `lon`).

---

## Bug 5 — Stop Fill Left SV2 Open Mid-Vent

**File:** `src/components/subcomponents/InitialFillComponent.tsx`

### What the Issue Was
The "INITIATE STOP FILL" button closed the Ball Valve and cleared `fillUIActive`, but did not check or close SV2. Since auto-vents and manual vents open SV2 on a timed basis, the operator could hit STOP FILL at any point during a vent cycle — including while SV2 was actively open. The fill loop would be killed (interval cleared) but SV2 would remain open indefinitely because the setTimeout that was supposed to close it references `handleButtonClickRef` in a closure that no longer triggers after the loop exits.

### How It Could Cause an Issue
The operator hits STOP FILL while a vent is in progress. The fill loop stops, BV closes, the screen returns to the idle state — but SV2 stays open. The run tank continues to vent with no operator awareness since the UI no longer shows any fill or vent activity. Depending on system pressure and how long this goes unnoticed, this could fully depressurize the tank.

### The Fix
The button now checks `isVentingRef.current` before closing BV and explicitly closes SV2 first if a vent is active:
```ts
if (isVentingRef.current) handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
handleButtonClickRef.current("Ball Valve", 'CLOSE');
isFillingRef.current = false;
isVentingRef.current = false;
setFillUIActive(false);
setVentUIActive(false);
```

---

## Bug 6 — Auto-Open SV2 on Connect Always Fired

**File:** `src/PropulsionPage.tsx`

### What the Issue Was
On connect, a 400ms delayed callback checked `valveDataRef.current.SV2.actuated` and sent `fsw_open_sv` if SV2 appeared closed. The FSW telemetry stream was started at t=333ms — but a stream start command only tells the server to begin sending; no telemetry response arrives in under 70ms. At t=400ms, `SV2.actuated` was always `false` (its initial state), so `fsw_open_sv` was sent unconditionally on every single connection regardless of actual SV2 state. The same logic applied to MAV — if MAV happened to be `false` in local state (which it always was on fresh connect), it was never auto-closed either even if it was actually open.

### How It Could Cause an Issue
Every time the operator navigates to the Propulsion Page or the WebSocket reconnects (e.g., brief network blip), an unsolicited `fsw_open_sv` command is sent to FSW. If SV2 was intentionally closed by the operator (e.g., after a fill procedure), reconnecting silently reopens it. This could cause an unintended vent during a period when the operator expects the system to be closed.

### The Fix
The entire auto-open/auto-close block was removed. Valve states are now only changed by explicit operator action. Initial states are populated by polling queries (SV1, BV, QD) and by the FSW telemetry stream once it starts delivering data.

---

## Bug 7 — Igniter Continuity Not Polled

**File:** `src/PropulsionPage.tsx`

### What the Issue Was
Igniter continuity (`get_igniter_continuity`) was queried once on initial connect but was not included in the 3-second polling interval. The poll only queried SV1, BV, and QD states. The igniter fire guard in `handleButtonClick` blocks the ignite command if `IG1.continuity || IG2.continuity` is false — but it reads from `valveDataRef`, which was never refreshed after the initial query.

### How It Could Cause an Issue
An igniter wire that was intact at page load develops a fault mid-session (vibration, connector issue). The UI continues to show continuity as good because the stale initial reading is never updated. The operator believes igniters are healthy and proceeds through the launch sequence. The ignite command fires but one or both igniters fail to light due to the continuity fault that the UI never detected.

Conversely, a continuity issue present at load that gets resolved (connector re-seated) would never show as resolved in the UI, potentially blocking launch unnecessarily.

### The Fix
`getIgniterContinuity1` and `getIgniterContinuity2` constants were defined and added to the 3-second polling interval, keeping continuity status live throughout the session.

---

## Bug 8 — SV2 Continuity Always Set to `undefined`

**File:** `src/PropulsionPage.tsx`

### What the Issue Was
In the `fsw_telemetry` message handler, SV2 state was updated like this:
```ts
SV2: { "actuated": data.telemetry.sv_open, "venting": data.telemetry.sv_open, "continuity": data.continuity }
```
`data.continuity` does not exist on `fsw_telemetry` responses — that field exists only on `valve_state` responses. So on every telemetry update, `SV2.continuity` was overwritten with `undefined`. Any logic that later read `SV2.continuity` would always see `undefined` (falsy).

### How It Could Cause an Issue
If any future logic gates an action on SV2 continuity (e.g., a pre-fill check), it would always fail even if continuity was physically present. Additionally, every telemetry update was silently corrupting the continuity field, making it harder to debug if a continuity check were ever added. The data was being irreversibly overwritten on every telemetry tick (~1 Hz).

### The Fix
Changed to spread `prevState.SV2` and only update the fields that telemetry actually provides:
```ts
SV2: { ...prevState.SV2, "actuated": data.telemetry.sv_open, "venting": data.telemetry.sv_open }
```
Continuity is now left untouched by telemetry updates and retains whatever value was last set by a `valve_state` response.

---

## Bug 9 — Recovery Page Heartbeat Interval Leaked on Reconnect

**File:** `src/RecoveryPage.tsx`

### What the Issue Was
`heartbeatInterval` was declared with `let` inside the `onOpen` callback function. The `useEffect` cleanup function (the `return () => { ... }` block) only removed the message listener — it never called `clearInterval`. Additionally, `heartbeatInterval` was scoped inside `onOpen`, so even if `clearInterval` had been called in cleanup, it could not have reached the variable.

### How It Could Cause an Issue
`useEffect([wsReady])` re-runs whenever `wsReady` changes — which happens every time the WebSocket disconnects and reconnects. Each reconnect calls `onOpen`, which creates a new `setInterval`. Without `clearInterval` in cleanup, the old interval is never cancelled. After 5 reconnects, 5 separate heartbeat intervals are all firing simultaneously, each sending a heartbeat every 5 seconds, resulting in 5 heartbeats per 5-second window instead of 1. Repeated reconnections (network instability at a launch site) would compound this indefinitely, flooding the server with heartbeats and making server-side timeout logic (which tracks heartbeat recency) unreliable.

### The Fix
`heartbeatInterval` was moved to the outer scope of the `useEffect` (outside `onOpen`) so the cleanup function can reference it. `clearInterval(heartbeatInterval)` was added to the cleanup return, and `removeEventListener('open', onOpen)` was also added (which was also missing).

---

---

## Bug 10 — No Heartbeat on Landing Page (Server Disconnects After 15s)

**File:** `src/App.tsx`

### What the Issue Was
The fill-station server (`main.rs:231`) disconnects any client that goes 15 seconds without sending a heartbeat. Heartbeats were only sent by `PropulsionPage` and `RecoveryPage` inside their own `useEffect` hooks — meaning the heartbeat only ran while those components were mounted. The `LandingPage` sent no heartbeat at all. `App.tsx`, which owns the WebSocket, also sent no heartbeat.

### How It Could Cause an Issue
The operator opens the app and sits on the LandingPage for more than 15 seconds (e.g., running through a pre-launch checklist, waiting for the team). The server silently disconnects the client. When the operator navigates to PropulsionPage, the app reconnects (3-second delay), re-queries valve states, and re-starts streams — but the operator may not notice the reconnect happened and could miss the brief window where state was unknown. In a worst case, if the reconnect occurs during a fill sequence the operator initiated before leaving the page, the state machine would restart with default values.

Additionally, page components individually managing heartbeats meant that during the instant React unmounts one page and mounts the next (navigation), there was a brief moment with no active heartbeat interval. With a 5-second heartbeat and a 15-second timeout this was safe in practice, but it was an unnecessary fragility.

### The Fix
The heartbeat was moved into `App.tsx` — it now starts in the WebSocket `onopen` handler and is cleared in `onclose`. This means a heartbeat is always sent every 5 seconds regardless of which page is active, and it is automatically stopped and restarted around reconnects. The page-level heartbeat intervals in `PropulsionPage` and `RecoveryPage` remain as redundant safety beats.

---

## SV2 State Source — Verified Correct (No Fix Needed)

**File:** `src/PropulsionPage.tsx`

### Verification
SV2 (`fsw_open_sv` / `fsw_close_sv`) is a command sent through the fill-station server to FSW via the umbilical serial port (`<S>` / `<s>` tokens in `main.rs:637-648`). The actual valve state is reported back by FSW in the `$TELEM` CSV stream, which the fill-station server parses into `FswTelemetry.sv_open` (`umbilical.rs:33`) and streams to connected clients as `fsw_telemetry` messages.

The UI correctly reads `data.telemetry.sv_open` from these messages and updates `valveData.SV2.actuated` only from that source (line 393). The `handleButtonClick` SV2 case sends the open/close command but makes no optimistic state update — it waits for the next telemetry packet to confirm the real state. This means SV2 displayed state always reflects what FSW actually reports, not what the UI last commanded.

The only requirement for this to work is that the FSW telemetry stream is running. `startFSWStream` is sent on connect in `PropulsionPage`'s `onOpen`, and the stream is stopped with `stopFSWStream` on unmount. This is handled correctly.

---

*Document generated 2026-04-28*
