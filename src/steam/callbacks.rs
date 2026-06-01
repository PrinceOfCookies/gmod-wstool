#![allow(unused)]

use super::client;
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};
use steamworks::{Callback, CallbackHandle};

pub type CallbackId = u64;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static CALLBACKS: LazyLock<Mutex<HashMap<CallbackId, CallbackHandle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static CHANNEL: LazyLock<(Sender<CallbackId>, Mutex<Receiver<CallbackId>>)> = LazyLock::new(|| {
    let (tx, rx) = channel();
    (tx, Mutex::new(rx))
});

fn next_id() -> CallbackId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn queue_remove(id: CallbackId) {
    let _ = CHANNEL.0.send(id);
}

/// Register a "normal" callback that stays alive until you explicitly unregister it.
///
/// Returns an id you can later use with `unregister`.
pub fn register<C, F>(f: F) -> CallbackId
where
    C: Callback,
    F: FnMut(C) + Send + 'static,
{
    let id = next_id();
    let handle = client().register_callback::<C, F>(f);
    CALLBACKS.lock().unwrap().insert(id, handle);
    id
}

/// Register a callback that runs **once** and then unregisters itself.
///
/// Returns its id (in case you want to cancel it before it fires).
pub fn register_once<C, F>(f: F) -> CallbackId
where
    C: Callback,
    F: FnMut(C) + Send + 'static,
{
    let id = next_id();
    let id_for_closure = id;
    let mut f = f;
    let handle = client().register_callback::<C, _>(move |event: C| {
        f(event);
        queue_remove(id_for_closure);
    });
    CALLBACKS.lock().unwrap().insert(id, handle);
    id
}

pub fn register_until<C, F>(mut f: F) -> CallbackId
where
    C: Callback,
    F: FnMut(C) -> bool + Send + 'static,
{
    let id = next_id();
    let id_for_closure = id;
    let handle = client().register_callback::<C, _>(move |event: C| {
        if f(event) {
            queue_remove(id_for_closure);
        }
    });
    CALLBACKS.lock().unwrap().insert(id, handle);
    id
}

pub fn flush_pending() -> usize {
    let rx = CHANNEL.1.lock().unwrap();
    let mut map = CALLBACKS.lock().unwrap();
    let mut count = 0;
    while let Ok(id) = rx.try_recv() {
        if map.remove(&id).is_some() {
            count += 1;
        }
    }
    count
}
