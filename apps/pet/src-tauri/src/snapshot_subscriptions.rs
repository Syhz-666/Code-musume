use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};
use tauri::{ipc::Channel, AppHandle, Manager};

const MAX_PENDING: usize = 8;
const OFFER_LIFETIME: Duration = Duration::from_secs(10);

#[derive(Serialize)]
#[serde(untagged)]
pub enum Update<T> {
    Offer { ticket: u64 },
    Snapshot { snapshot: T },
}

struct Subscription<T> {
    token: String,
    ticket: u64,
    offered_at: Instant,
    channel: Channel<Update<T>>,
}

struct WindowSubscriptions<T> {
    active: Option<Subscription<T>>,
    pending: VecDeque<Subscription<T>>,
}

impl<T> Default for WindowSubscriptions<T> {
    fn default() -> Self {
        Self {
            active: None,
            pending: VecDeque::new(),
        }
    }
}

pub struct SnapshotSubscriptions<T> {
    windows: BTreeMap<String, WindowSubscriptions<T>>,
    next_ticket: u64,
}

impl<T> Default for SnapshotSubscriptions<T> {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            next_ticket: 0,
        }
    }
}

impl<T: Clone + Serialize> SnapshotSubscriptions<T> {
    pub fn offer(
        &mut self,
        window: &str,
        token: String,
        make_channel: impl FnOnce() -> Channel<Update<T>>,
    ) -> (u64, &'static str) {
        let slot = self.windows.entry(window.into()).or_default();
        let now = Instant::now();
        slot.pending
            .retain(|s| now.duration_since(s.offered_at) < OFFER_LIFETIME);
        if let Some(existing) = slot
            .active
            .iter()
            .chain(&slot.pending)
            .find(|s| s.token == token)
        {
            // A fallback IPC invocation can repeat the command for the same Channel.
            return (existing.ticket, "offer-replayed");
        }

        self.next_ticket += 1;
        let ticket = self.next_ticket;
        let channel = make_channel();
        if channel.send(Update::Offer { ticket }).is_err() {
            return (ticket, "offer-send-failed");
        }
        if slot.pending.len() == MAX_PENDING {
            slot.pending.pop_front();
        }
        slot.pending.push_back(Subscription {
            token,
            ticket,
            offered_at: now,
            channel,
        });
        (ticket, "offered")
    }

    pub fn confirm(&mut self, window: &str, token: &str, ticket: u64) -> bool {
        let Some(slot) = self.windows.get_mut(window) else {
            return false;
        };
        if let Some(active) = &slot.active {
            if ticket <= active.ticket {
                return ticket == active.ticket && token == active.token;
            }
        }
        let now = Instant::now();
        let Some(index) = slot.pending.iter().position(|s| {
            s.token == token
                && s.ticket == ticket
                && now.duration_since(s.offered_at) < OFFER_LIFETIME
        }) else {
            return false;
        };
        slot.active = slot.pending.remove(index);
        // Delayed confirmations of any earlier offer cannot replace this page.
        slot.pending.retain(|s| s.ticket > ticket);
        true
    }

    pub fn cancel(&mut self, window: &str, token: &str) {
        if let Some(slot) = self.windows.get_mut(window) {
            if slot.active.as_ref().is_some_and(|s| s.token == token) {
                slot.active = None;
            }
            slot.pending.retain(|s| s.token != token);
        }
    }

    pub fn publish(
        &mut self,
        snapshot: &T,
        app: Option<&AppHandle>,
        skip_hidden: bool,
        mut record: impl FnMut(&str, u64, &'static str),
    ) -> usize {
        let mut active = 0;
        for (label, slot) in &mut self.windows {
            let Some(subscription) = &slot.active else {
                continue;
            };
            let (keep, outcome) = if let Some(app) = app {
                match app.get_webview_window(label) {
                    None => (false, "window-closed"),
                    Some(window) if skip_hidden && !window.is_visible().unwrap_or(false) => {
                        (true, "window-hidden")
                    }
                    Some(_) => send(subscription, snapshot),
                }
            } else {
                send(subscription, snapshot)
            };
            record(label, subscription.ticket, outcome);
            if keep {
                active += 1;
            } else {
                slot.active = None;
            }
        }
        active
    }
}

fn send<T: Clone + Serialize>(
    subscription: &Subscription<T>,
    snapshot: &T,
) -> (bool, &'static str) {
    if subscription
        .channel
        .send(Update::Snapshot {
            snapshot: snapshot.clone(),
        })
        .is_ok()
    {
        (true, "queued")
    } else {
        (false, "send-failed")
    }
}
