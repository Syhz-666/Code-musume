use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdleGate {
    Blocked,
    Paused,
    Ready,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleDiagnostics {
    rolls: u64,
    last_face: Option<u8>,
    rests: u64,
    wakes: u64,
    pub(super) requests: u64,
    pub(super) shown: u64,
    pub(super) last_skip: Option<&'static str>,
}

#[derive(Default)]
pub(super) struct IdleBanter {
    pub enabled: bool,
    pub resting: bool,
    pub recent: VecDeque<String>,
    pub diagnostics: IdleDiagnostics,
    next_roll: Option<u64>,
    streak: u8,
}

impl IdleBanter {
    fn interval(&self) -> u64 {
        if self.resting {
            15_000
        } else {
            30_000
        }
    }

    pub fn update(&mut self, gate: IdleGate, now: u64) {
        if gate == IdleGate::Ready {
            let next = now + self.interval();
            self.next_roll.get_or_insert(next);
            return;
        }
        self.next_roll = None;
        self.streak = 0;
        if gate == IdleGate::Blocked && self.resting {
            self.resting = false;
            self.diagnostics.wakes += 1;
        }
    }

    pub fn roll(&mut self, now: u64, dice: impl FnOnce() -> u8) -> bool {
        let Some(due) = self.next_roll.filter(|&due| now >= due) else {
            return false;
        };
        self.next_roll = Some(now + self.interval());
        // Give the host timer a small tolerance. Discard old deadlines after suspend.
        if now - due > 1_000 {
            self.streak = 0;
            self.diagnostics.last_skip = Some("missed-deadline");
            return false;
        }
        let face = dice();
        self.diagnostics.rolls += 1;
        self.diagnostics.last_face = Some(face);
        self.diagnostics.last_skip = None;
        let streak_face = if self.resting { 6 } else { 1 };
        self.streak = if face == streak_face {
            self.streak + 1
        } else {
            0
        };
        let threshold = if self.resting { 2 } else { 3 };
        if self.streak == threshold {
            self.resting = !self.resting;
            self.streak = 0;
            self.next_roll = Some(now + self.interval());
            if self.resting {
                self.diagnostics.rests += 1;
            } else {
                self.diagnostics.wakes += 1;
            }
            return false;
        }
        !self.resting && face == 6
    }
}
