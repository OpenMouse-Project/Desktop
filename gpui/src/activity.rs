//! Whether the main window is in front, shared with the background loops.
//!
//! The loops that poll the system (game processes, launcher installs, the
//! wallpaper) have no reason to run at full rate while nobody is looking at
//! the window, but they cannot simply stop: a game launching behind the
//! window must still apply its profile. So they keep running at a slower
//! cadence while the window is not active, and [`Activity::sleep`] cuts the
//! slow wait short the moment the window comes back so what is on screen is
//! fresh when it appears.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use futures::channel::oneshot;
use futures::future::select;
use gpui_kit::BackgroundExecutor;

pub struct Activity {
    active: AtomicBool,
    waiters: Mutex<Vec<oneshot::Sender<()>>>,
}

/// The main window's state. Starts active so nothing is slowed before the
/// window has reported anything.
pub static ACTIVITY: Activity = Activity::new();

impl Activity {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
            waiters: Mutex::new(Vec::new()),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Records the window's state; becoming active wakes every slow sleeper.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
        if active {
            let waiters = std::mem::take(&mut *self.waiters.lock().unwrap());
            for waiter in waiters {
                let _ = waiter.send(());
            }
        }
    }

    /// A receiver that resolves when the window becomes active, or `None` if
    /// it already is. The check and the registration happen under one lock so
    /// a wake-up between them cannot be missed.
    fn until_active(&self) -> Option<oneshot::Receiver<()>> {
        let mut waiters = self.waiters.lock().unwrap();
        if self.is_active() {
            return None;
        }
        waiters.retain(|waiter| !waiter.is_canceled());
        let (sender, receiver) = oneshot::channel();
        waiters.push(sender);
        Some(receiver)
    }

    /// Waits `active_for` while the window is active, `inactive_for` while it
    /// is not, ending early if it becomes active during the longer wait.
    pub async fn sleep(
        &self,
        executor: &BackgroundExecutor,
        active_for: Duration,
        inactive_for: Duration,
    ) {
        match self.until_active() {
            None => executor.timer(active_for).await,
            Some(woken) => {
                let timer = executor.timer(inactive_for);
                futures::pin_mut!(timer);
                let _ = select(timer, woken).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn becoming_active_wakes_a_registered_sleeper() {
        let activity = Activity::new();
        activity.set_active(false);
        let mut woken = activity.until_active().expect("inactive window registers a waiter");
        assert_eq!(woken.try_recv(), Ok(None), "not woken while still inactive");
        activity.set_active(true);
        assert_eq!(woken.try_recv(), Ok(Some(())));
    }

    #[test]
    fn an_active_window_never_registers_a_waiter() {
        let activity = Activity::new();
        assert!(activity.until_active().is_none());
        assert!(activity.waiters.lock().unwrap().is_empty());
    }

    #[test]
    fn abandoned_waiters_are_dropped_on_the_next_registration() {
        let activity = Activity::new();
        activity.set_active(false);
        drop(activity.until_active());
        drop(activity.until_active());
        let _kept = activity.until_active();
        assert_eq!(activity.waiters.lock().unwrap().len(), 1);
    }
}
