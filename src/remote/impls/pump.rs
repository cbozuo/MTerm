//! The I/O → UI crossing point for remote-desktop tabs.
//!
//! Slint may only be touched from the UI thread, and a protocol session runs on
//! a tokio task, so every frame and state change has to be handed over. This
//! module owns that hand-over for all remote-desktop tabs at once: one pump
//! thread per window drains each session's event channel and applies the events
//! to the window's `RemotePaneState` model.
//!
//! Two properties matter and are enforced here rather than by convention:
//!
//! * **boundedness** — [`FrameQueue`] keeps at most `FRAME_QUEUE_DEPTH` frames
//!   pending and drops the *oldest* one when full. A desktop that redraws
//!   faster than the UI can upload must never make the protocol task wait, and
//!   the user must never be shown a frame that is already seconds stale.
//! * **routing** — the pump resolves the target window per event, so a tab
//!   dragged to another window keeps receiving frames without reconnecting
//!   (US-K2), the same way the terminal pumps retarget through `TabRoute`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::remote::pane::{RemoteEvent, FRAME_QUEUE_DEPTH};

/// Where a tab's remote events are currently delivered.
///
/// Only the window id lives here: it is plain data and therefore `Send`. A
/// `slint::Weak` is not `Send`, so storing one in a map that protocol threads
/// hold would make every `spawn` that captures it fail to compile. The window
/// itself is looked up on the UI thread by id, which is where `Weak` may be
/// touched anyway.
#[derive(Clone)]
pub struct RemoteRoute {
    pub window_id: u64,
}

/// Tab id → its current delivery route, shared with the protocol tasks.
pub type RemoteRoutes = Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<RemoteRoute>>>>>;

/// Create the channel a session publishes its events on.
///
/// Unbounded on purpose: the *frames* are bounded by [`FrameQueue`] before they
/// reach it, and state/certificate events are rare and must never be dropped.
pub fn event_channel() -> (mpsc::UnboundedSender<RemoteEvent>, mpsc::UnboundedReceiver<RemoteEvent>) {
    mpsc::unbounded_channel()
}

/// A bounded, drop-oldest queue in front of the UI thread.
///
/// Frames go through here rather than straight into a bounded channel because
/// tokio's bounded channel makes a *full* producer wait (or fail), and a
/// protocol task waiting on the UI thread is exactly the deadlock this design
/// exists to avoid: the UI thread is itself waiting on that task's output.
pub struct FrameQueue {
    queue: VecDeque<RemoteEvent>,
    capacity: usize,
}

impl FrameQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(FRAME_QUEUE_DEPTH),
            capacity: FRAME_QUEUE_DEPTH,
        }
    }

    /// Queue a frame, discarding the oldest if the queue is already full.
    pub fn push_frame(&mut self, frame: RemoteEvent) {
        while self.queue.len() >= self.capacity {
            self.queue.pop_front();
        }
        self.queue.push_back(frame);
    }

    /// Queue an event that must not be lost (state, certificate prompt).
    ///
    /// It still goes through the same bounded queue so a burst of state changes
    /// cannot grow it without limit. When there is no room the oldest entry goes
    /// — a frame if one is at the front (the freshest frame is what matters, not
    /// the oldest), otherwise the oldest state, which is the one superseded by
    /// the event being pushed.
    pub fn push_control(&mut self, event: RemoteEvent) {
        while self.queue.len() >= self.capacity {
            self.queue.pop_front();
        }
        self.queue.push_back(event);
    }

    /// Take everything queued so far, leaving the queue empty.
    pub fn drain(&mut self) -> Vec<RemoteEvent> {
        self.queue.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl Default for FrameQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the pump for one session.
///
/// Blocks on the channel, so exactly one pump exists per session; it exits when
/// the protocol task drops its sender, i.e. when the session ends.
pub fn spawn_pump(
    tab_id: String,
    mut rx: mpsc::UnboundedReceiver<RemoteEvent>,
    routes: RemoteRoutes,
    shutdown: Arc<AtomicBool>,
) {
    // The handle is deliberately dropped: the pump exits when its channel
    // closes, and nothing has any reason to join it.
    let _ = std::thread::Builder::new()
        .name(format!("remote-pump-{tab_id}"))
        .spawn(move || {
            while let Some(event) = rx.blocking_recv() {
                // The flag is read *after* the event is handled, not before.
                // A driver sets it immediately before publishing its final state
                // — "disconnected" or "failed" — so checking first dropped
                // exactly the event the UI most needs, and the tab sat on its
                // previous status ("connected") for a session that had already
                // ended. One extra event is harmless: `apply_to` is a no-op once
                // the tab or its window is gone, while losing the last state is
                // not.
                let stopping = shutdown.load(Ordering::Acquire);
                // Resolve the route per event: a tab moved to another window
                // mid-stream delivers there from the next frame on.
                let route = routes
                    .lock()
                    .ok()
                    .and_then(|map| map.get(&tab_id).cloned())
                    .and_then(|route| route.lock().ok().map(|r| r.clone()));
                if let Some(route) = route {
                    // Only the id crosses into the event loop. The lookup happens
                    // inside the closure, on the UI thread, because `Weak` may not
                    // be created, cloned or upgraded anywhere else.
                    let window_id = route.window_id;
                    let tab = tab_id.clone();
                    // `invoke_from_event_loop` queues the closure on the event
                    // loop, so it still runs after this thread has exited.
                    let _ = slint::invoke_from_event_loop(move || {
                        // Only the id crosses into the event loop; the thread-local
                        // pane table is read here, where it is legal to.
                        crate::remote::registry::apply_to(window_id, &tab, event);
                    });
                }
                // No window wants these events any more (tab closed): nothing
                // left to deliver, so stop.
                if stopping {
                    break;
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn frame() -> RemoteEvent {
        RemoteEvent::Frame(RgbaImage::new(4, 4))
    }

    #[test]
    fn the_queue_keeps_only_the_newest_frames() {
        let mut q = FrameQueue::new();
        q.push_frame(frame());
        q.push_frame(frame());
        q.push_frame(frame());
        // At capacity: the third push evicted the first.
        assert_eq!(q.len(), FRAME_QUEUE_DEPTH);
    }

    #[test]
    fn a_state_change_never_gets_dropped_in_favour_of_a_stale_frame() {
        let mut q = FrameQueue::new();
        q.push_frame(frame());
        q.push_frame(frame());
        q.push_control(RemoteEvent::State {
            state: Arc::new(crate::rdp::embedded::status::RemoteStatus::default()),
            message: Some("failed".into()),
        });
        let drained = q.drain();
        assert_eq!(drained.len(), FRAME_QUEUE_DEPTH);
        assert!(
            drained.iter().any(|e| matches!(e, RemoteEvent::State { .. })),
            "the state change must survive a full queue"
        );
    }

    #[test]
    fn draining_empties_the_queue_so_the_next_batch_starts_fresh() {
        let mut q = FrameQueue::new();
        q.push_frame(frame());
        assert_eq!(q.drain().len(), 1);
        assert!(q.is_empty());
        assert!(q.drain().is_empty());
    }
}
