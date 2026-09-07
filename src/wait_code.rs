//! Shared batch poller for [`wait_code`](crate::api::numbers::NumbersApi::wait_code).
//!
//! Multiple concurrent `wait_code` calls share one background loop that polls
//! `getState` **without** `tzid` (all active numbers) and fans results out by
//! operation id. The loop starts on the first waiter and stops when none remain.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::types::numbers::{StateOne, WaitCodeOptions};

pub(crate) fn msg_to_code(msg: Value) -> String {
    match msg {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

struct Waiter<T> {
    tzid: i64,
    options: WaitCodeOptions,
    attempts: u32,
    last_code: String,
    tx: Option<T>,
}

struct Inner<T> {
    waiters: Vec<Waiter<T>>,
    running: bool,
}

impl<T> Default for Inner<T> {
    fn default() -> Self {
        Self {
            waiters: Vec::new(),
            running: false,
        }
    }
}

fn poll_params<T>(waiters: &[Waiter<T>]) -> (u64, i64) {
    let interval = waiters
        .iter()
        .map(|w| w.options.interval_secs)
        .min()
        .unwrap_or(3);
    let full = waiters.iter().any(|w| w.options.full_message);
    (interval, if full { 0 } else { 1 })
}

/// Outcomes of matching a `getState` list against active waiters.
struct TickOutcome<T> {
    /// Deliveries ready to send after next/close.
    deliveries: Vec<(i64, String, T, bool)>,
}

fn match_states<T>(waiters: &mut Vec<Waiter<T>>, states: &[StateOne]) -> TickOutcome<T> {
    let mut by_tzid: HashMap<i64, &StateOne> = HashMap::new();
    for s in states {
        by_tzid.insert(s.tzid, s);
    }

    let mut deliveries = Vec::new();
    let mut i = 0;
    while i < waiters.len() {
        let w = &mut waiters[i];
        let code = by_tzid
            .get(&w.tzid)
            .and_then(|s| s.msg.clone())
            .map(msg_to_code);
        if let Some(code) = code {
            if code != w.last_code {
                let not_end = w.options.not_end;
                let tzid = w.tzid;
                let tx = w.tx.take().expect("waiter tx");
                waiters.remove(i);
                deliveries.push((tzid, code, tx, not_end));
                continue;
            }
        }
        i += 1;
    }

    TickOutcome { deliveries }
}

fn bump_attempts<T>(waiters: &mut Vec<Waiter<T>>) -> Vec<T> {
    let mut timeouts = Vec::new();
    waiters.retain_mut(|w| {
        w.attempts = w.attempts.saturating_add(1);
        if w.attempts > w.options.max_attempts {
            if let Some(tx) = w.tx.take() {
                timeouts.push(tx);
            }
            false
        } else {
            true
        }
    });
    timeouts
}

#[cfg(feature = "async")]
pub mod async_poller {
    use super::*;
    use crate::http::Http;
    use tokio::sync::oneshot;

    type Tx = oneshot::Sender<Result<String>>;

    /// Shared async wait-code poller (one per [`crate::Client`]).
    #[derive(Clone, Default)]
    pub struct AsyncWaitPoller {
        inner: Arc<Mutex<Inner<Tx>>>,
    }

    impl fmt::Debug for AsyncWaitPoller {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("AsyncWaitPoller")
        }
    }

    impl AsyncWaitPoller {
        /// Register `tzid` and wait until a code arrives (or timeout / poll error).
        pub async fn wait_code(
            &self,
            http: Http,
            tzid: i64,
            options: WaitCodeOptions,
        ) -> Result<String> {
            let (tx, rx) = oneshot::channel();
            let start = {
                let mut g = self.inner.lock().expect("wait poller");
                g.waiters.push(Waiter {
                    tzid,
                    options,
                    attempts: 0,
                    last_code: String::new(),
                    tx: Some(tx),
                });
                if g.running {
                    false
                } else {
                    g.running = true;
                    true
                }
            };
            if start {
                let inner = self.inner.clone();
                tokio::spawn(async move {
                    run_loop(inner, http).await;
                });
            }
            match rx.await {
                Ok(res) => res,
                Err(_) => Err(Error::Unexpected(
                    "wait_code cancelled: poller dropped".into(),
                )),
            }
        }
    }

    async fn run_loop(inner: Arc<Mutex<Inner<Tx>>>, http: Http) {
        loop {
            let (interval, message_to_code) = {
                let mut g = inner.lock().expect("wait poller");
                if g.waiters.is_empty() {
                    g.running = false;
                    return;
                }
                poll_params(&g.waiters)
            };

            tokio::time::sleep(Duration::from_secs(interval)).await;

            let timeouts = {
                let mut g = inner.lock().expect("wait poller");
                bump_attempts(&mut g.waiters)
            };
            for tx in timeouts {
                let _ = tx.send(Err(Error::Timeout));
            }

            {
                let mut g = inner.lock().expect("wait poller");
                if g.waiters.is_empty() {
                    g.running = false;
                    return;
                }
            }

            let states: std::result::Result<Vec<StateOne>, Error> = http
                .get_onlinesim(
                    "getState",
                    json!({
                        "message_to_code": message_to_code,
                        "orderby": "asc",
                        "msg_list": 0,
                        "clean": 1,
                        "type": "index",
                    }),
                    true,
                )
                .await;

            let states = match states {
                Ok(s) => s,
                Err(e) => {
                    fail_all(&inner, e);
                    return;
                }
            };

            let outcome = {
                let mut g = inner.lock().expect("wait poller");
                match_states(&mut g.waiters, &states)
            };

            // next/close once per tzid, then deliver.
            let mut done_tzid = HashMap::<i64, Option<String>>::new();
            for (tzid, _code, _tx, not_end) in &outcome.deliveries {
                if done_tzid.contains_key(tzid) {
                    continue;
                }
                let err = if *not_end {
                    http.get_onlinesim("setOperationRevise", json!({ "tzid": tzid }), true)
                        .await
                        .map(|_: Value| ())
                        .err()
                        .map(|e| e.to_string())
                } else {
                    http.get_onlinesim("setOperationOk", json!({ "tzid": tzid }), true)
                        .await
                        .map(|_: Value| ())
                        .err()
                        .map(|e| e.to_string())
                };
                done_tzid.insert(*tzid, err);
            }

            for (tzid, code, tx, _) in outcome.deliveries {
                let res = match done_tzid.get(&tzid) {
                    Some(None) | None => Ok(code),
                    Some(Some(e)) => {
                        Err(Error::Unexpected(format!("wait_code finish failed: {e}")))
                    }
                };
                let _ = tx.send(res);
            }
        }
    }

    fn fail_all(inner: &Arc<Mutex<Inner<Tx>>>, err: Error) {
        let msg = err.to_string();
        let mut g = inner.lock().expect("wait poller");
        for w in g.waiters.drain(..) {
            if let Some(tx) = w.tx {
                let _ = tx.send(Err(Error::Unexpected(format!(
                    "wait_code poll failed: {msg}"
                ))));
            }
        }
        g.running = false;
    }
}

#[cfg(feature = "blocking")]
pub mod blocking_poller {
    use super::*;
    use crate::blocking::http::BlockingHttp;
    use std::sync::mpsc;
    use std::thread;

    type Tx = mpsc::SyncSender<Result<String>>;

    /// Shared blocking wait-code poller (one thread per [`crate::blocking::Client`]).
    #[derive(Clone, Default)]
    pub struct BlockingWaitPoller {
        inner: Arc<Mutex<Inner<Tx>>>,
    }

    impl fmt::Debug for BlockingWaitPoller {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("BlockingWaitPoller")
        }
    }

    impl BlockingWaitPoller {
        /// Register `tzid` and block until a code arrives (or timeout / poll error).
        pub fn wait_code(
            &self,
            http: BlockingHttp,
            tzid: i64,
            options: WaitCodeOptions,
        ) -> Result<String> {
            let (tx, rx) = mpsc::sync_channel(1);
            let start = {
                let mut g = self.inner.lock().expect("wait poller");
                g.waiters.push(Waiter {
                    tzid,
                    options,
                    attempts: 0,
                    last_code: String::new(),
                    tx: Some(tx),
                });
                if g.running {
                    false
                } else {
                    g.running = true;
                    true
                }
            };
            if start {
                let inner = self.inner.clone();
                thread::spawn(move || run_loop(inner, http));
            }
            match rx.recv() {
                Ok(res) => res,
                Err(_) => Err(Error::Unexpected(
                    "wait_code cancelled: poller dropped".into(),
                )),
            }
        }
    }

    fn run_loop(inner: Arc<Mutex<Inner<Tx>>>, http: BlockingHttp) {
        loop {
            let (interval, message_to_code) = {
                let mut g = inner.lock().expect("wait poller");
                if g.waiters.is_empty() {
                    g.running = false;
                    return;
                }
                poll_params(&g.waiters)
            };

            thread::sleep(Duration::from_secs(interval));

            let timeouts = {
                let mut g = inner.lock().expect("wait poller");
                bump_attempts(&mut g.waiters)
            };
            for tx in timeouts {
                let _ = tx.send(Err(Error::Timeout));
            }

            {
                let mut g = inner.lock().expect("wait poller");
                if g.waiters.is_empty() {
                    g.running = false;
                    return;
                }
            }

            let states: std::result::Result<Vec<StateOne>, Error> = http.get_onlinesim(
                "getState",
                json!({
                    "message_to_code": message_to_code,
                    "orderby": "asc",
                    "msg_list": 0,
                    "clean": 1,
                    "type": "index",
                }),
                true,
            );

            let states = match states {
                Ok(s) => s,
                Err(e) => {
                    fail_all(&inner, e);
                    return;
                }
            };

            let outcome = {
                let mut g = inner.lock().expect("wait poller");
                match_states(&mut g.waiters, &states)
            };

            let mut done_tzid = HashMap::<i64, Option<String>>::new();
            for (tzid, _code, _tx, not_end) in &outcome.deliveries {
                if done_tzid.contains_key(tzid) {
                    continue;
                }
                let err = if *not_end {
                    http.get_onlinesim("setOperationRevise", json!({ "tzid": tzid }), true)
                        .map(|_: Value| ())
                        .err()
                        .map(|e| e.to_string())
                } else {
                    http.get_onlinesim("setOperationOk", json!({ "tzid": tzid }), true)
                        .map(|_: Value| ())
                        .err()
                        .map(|e| e.to_string())
                };
                done_tzid.insert(*tzid, err);
            }

            for (tzid, code, tx, _) in outcome.deliveries {
                let res = match done_tzid.get(&tzid) {
                    Some(None) | None => Ok(code),
                    Some(Some(e)) => {
                        Err(Error::Unexpected(format!("wait_code finish failed: {e}")))
                    }
                };
                let _ = tx.send(res);
            }
        }
    }

    fn fail_all(inner: &Arc<Mutex<Inner<Tx>>>, err: Error) {
        let msg = err.to_string();
        let mut g = inner.lock().expect("wait poller");
        for w in g.waiters.drain(..) {
            if let Some(tx) = w.tx {
                let _ = tx.send(Err(Error::Unexpected(format!(
                    "wait_code poll failed: {msg}"
                ))));
            }
        }
        g.running = false;
    }
}
