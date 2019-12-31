use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct Metrics {
    tcp_requests: AtomicU64,
    udp_requests: AtomicU64,
    local_requests: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            tcp_requests: AtomicU64::new(0),
            udp_requests: AtomicU64::new(0),
            local_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        })
    }

    pub fn tcp_requests(&self) -> u64 {
        self.tcp_requests.load(Ordering::Relaxed)
    }

    pub fn increment_tcp_requests(&self) {
        self.tcp_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn local_requests(&self) -> u64 {
        self.local_requests.load(Ordering::Relaxed)
    }

    pub fn increment_local_requests(&self) {
        self.local_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn udp_requests(&self) -> u64 {
        self.udp_requests.load(Ordering::Relaxed)
    }

    pub fn increment_udp_requests(&self) {
        self.udp_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    pub fn increment_cache_hits(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn cache_misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }

    pub fn increment_cache_misses(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
}

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tcp_requests = {} udp_requests = {} local_requests = {} cache_hits = {} cache_misses = {}",
            self.tcp_requests(),
            self.udp_requests(),
            self.local_requests(),
            self.cache_hits(),
            self.cache_misses()
        )
    }
}
