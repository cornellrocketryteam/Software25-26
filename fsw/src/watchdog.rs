//! Global watchdog accessor.
//!
//! The hardware watchdog lives in a `Mutex<RefCell<Option<Watchdog>>>` so long
//! inline operations (flash erase/wipe/dump) can pet it between sub-steps
//! without having to thread `&mut Watchdog` through every call site.
//!
//! Usage:
//!   - In `main`, call [`init`] once with `Watchdog::new(p.WATCHDOG)`.
//!   - In the flight loop, call [`feed`] around `execute()` as before.
//!   - Inside long-running inline ops (e.g. `wipe_storage`), call [`feed`]
//!     between sub-steps so the chip isn't reset mid-operation.

use core::cell::RefCell;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::Duration;

use crate::constants;

static WATCHDOG: Mutex<CriticalSectionRawMutex, RefCell<Option<Watchdog>>> =
    Mutex::new(RefCell::new(None));

/// Install the global watchdog and start its countdown. Call exactly once
/// from `main` before entering the flight loop.
pub fn init(mut wd: Watchdog) {
    wd.start(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));
    WATCHDOG.lock(|cell| {
        *cell.borrow_mut() = Some(wd);
    });
}

/// Reset the watchdog's countdown back to `WATCHDOG_TIMEOUT_MS`. Safe to call
/// from any task/context. No-op if [`init`] hasn't been called (e.g. in
/// simulation builds that skip the flight loop).
pub fn feed() {
    WATCHDOG.lock(|cell| {
        if let Some(wd) = cell.borrow_mut().as_mut() {
            wd.feed(Duration::from_millis(constants::WATCHDOG_TIMEOUT_MS as u64));
        }
    });
}
