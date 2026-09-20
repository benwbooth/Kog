//! Connected-device registry.
//!
//! The API's clients are browsers and remote players, and until now the
//! server had no idea any of them existed beyond a log line. This registry
//! records one entry per client — identified by the `X-Kog-Device` header
//! the web player sends, falling back to the client's user agent — so the
//! desktop's settings can show who is connected and cut a device off.
//! Blocking is enforced at the API boundary: a blocked device's `/api`
//! requests are refused before they reach any handler.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// One connected client: when it appeared, what it has asked for, and
/// whether the user has cut it off.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DeviceEntry {
    pub id: String,
    pub agent: String,
    pub addr: String,
    /// Epoch milliseconds.
    pub first_seen: u64,
    pub last_seen: u64,
    pub requests: u64,
    pub blocked: bool,
}

#[derive(Default)]
pub struct Devices {
    entries: HashMap<String, DeviceEntry>,
}

/// Keep the registry bounded: entries unseen for a week fall off, and the
/// map never grows past this many regardless.
const MAX_DEVICES: usize = 200;
const STALE_MS: u64 = 60 * 60 * 24 * 7 * 1000;

impl Devices {
    /// Note one request from a device, creating or refreshing its entry.
    pub fn record(&mut self, id: String, agent: String, addr: String) {
        let now = now_ms();
        match self.entries.get_mut(&id) {
            Some(entry) => {
                entry.agent = agent;
                entry.addr = addr;
                entry.last_seen = now;
                entry.requests += 1;
            }
            None => {
                self.entries.insert(
                    id.clone(),
                    DeviceEntry {
                        id,
                        agent,
                        addr,
                        first_seen: now,
                        last_seen: now,
                        requests: 1,
                        blocked: false,
                    },
                );
            }
        }
        self.prune(now);
    }

    fn prune(&mut self, now: u64) {
        if self.entries.len() <= MAX_DEVICES {
            return;
        }
        self.entries
            .retain(|_, entry| now.saturating_sub(entry.last_seen) < STALE_MS);
    }

    /// Cut a device off or let it back in. `false` when nothing is known
    /// under that id.
    pub fn set_blocked(&mut self, id: &str, blocked: bool) -> bool {
        match self.entries.get_mut(id) {
            Some(entry) => {
                entry.blocked = blocked;
                true
            }
            None => false,
        }
    }

    pub fn blocked(&self, id: &str) -> bool {
        self.entries.get(id).is_some_and(|entry| entry.blocked)
    }

    /// Most recently active first.
    pub fn list(&self) -> Vec<DeviceEntry> {
        let mut list: Vec<DeviceEntry> = self.entries.values().cloned().collect();
        list.sort_by(|left, right| right.last_seen.cmp(&left.last_seen));
        list
    }
}

/// The process-wide registry. The desktop embeds the API server in its own
/// process, so its settings read this directly; the standalone server binary
/// gets the same one.
pub fn registry() -> MutexGuard<'static, Devices> {
    static REGISTRY: OnceLock<Mutex<Devices>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| Mutex::new(Devices::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
