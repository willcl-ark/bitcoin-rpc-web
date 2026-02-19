use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RpcLimiter {
    max_in_flight: usize,
    in_flight: AtomicUsize,
}

pub struct RpcPermit {
    limiter: Arc<RpcLimiter>,
}

impl RpcLimiter {
    pub fn new(max_in_flight: usize) -> Arc<Self> {
        Arc::new(Self {
            max_in_flight,
            in_flight: AtomicUsize::new(0),
        })
    }

    pub fn try_acquire(self: &Arc<Self>) -> Option<RpcPermit> {
        let mut current = self.in_flight.load(Ordering::Relaxed);
        loop {
            if current >= self.max_in_flight {
                return None;
            }
            match self.in_flight.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(RpcPermit {
                        limiter: Arc::clone(self),
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }
}

impl Drop for RpcPermit {
    fn drop(&mut self) {
        self.limiter.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}
