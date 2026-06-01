use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;

pub struct AsyncLoad<T, P = ()> {
    rx: RefCell<Option<Receiver<T>>>,
    job: Arc<dyn Fn(P) -> T + Send + Sync + 'static>,
    result: RefCell<Option<Rc<T>>>,
}

impl<T: Send + 'static, P: Send + 'static> AsyncLoad<T, P> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(P) -> T + Send + Sync + 'static,
    {
        Self {
            rx: RefCell::new(None),
            job: Arc::new(f),
            result: RefCell::new(None),
        }
    }

    fn spawn_job(&self, params: P) {
        let (tx, rx) = mpsc::channel::<T>();
        *self.rx.borrow_mut() = Some(rx);
        let job = Arc::clone(&self.job);
        thread::spawn(move || {
            let _ = tx.send(job(params));
        });
    }

    fn try_recv(&self) -> Option<Rc<T>> {
        // Fast path: already cached?
        if let Some(cached) = self.result.borrow().as_ref() {
            return Some(cached.clone());
        }
        // try_recv in its own scope so the borrow drops before we mutate rx
        let received = {
            let guard = self.rx.borrow();
            guard.as_ref()?.try_recv().ok()
        };
        let value = received?;
        // job is done, drop the receiver so update() won't respawn
        *self.rx.borrow_mut() = None;
        let rc = Rc::new(value);
        *self.result.borrow_mut() = Some(rc.clone());
        Some(rc)
    }

    pub fn reset(&self, params: P) {
        *self.result.borrow_mut() = None;
        self.spawn_job(params);
    }

    pub fn update(&self, params: P) -> Option<Rc<T>> {
        if let Some(res) = self.try_recv() {
            return Some(res);
        }
        if self.rx.borrow().is_none() {
            self.spawn_job(params);
        }
        None
    }

    /// Check for result, spawn lazily if not started
    #[allow(unused)]
    pub fn update_lazy<F>(&self, params_fn: F) -> Option<Rc<T>>
    where
        F: FnOnce() -> P,
    {
        if let Some(res) = self.try_recv() {
            return Some(res);
        }

        if self.rx.borrow().is_none() {
            self.spawn_job(params_fn());
        }

        None
    }
}
