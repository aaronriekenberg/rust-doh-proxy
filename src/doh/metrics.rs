use std::fmt;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct AtomicU64Metric {
    value: AtomicU64,
}

impl AtomicU64Metric {
    fn new() -> Self {
        AtomicU64Metric {
            value: AtomicU64::new(0),
        }
    }

    pub fn increment_value(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

pub struct Metrics {
    tcp_requests: AtomicU64Metric,
    udp_requests: AtomicU64Metric,
    local_requests: AtomicU64Metric,
    cache_hits: AtomicU64Metric,
    cache_misses: AtomicU64Metric,
    doh_request_errors: AtomicU64Metric,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            tcp_requests: AtomicU64Metric::new(),
            udp_requests: AtomicU64Metric::new(),
            local_requests: AtomicU64Metric::new(),
            cache_hits: AtomicU64Metric::new(),
            cache_misses: AtomicU64Metric::new(),
            doh_request_errors: AtomicU64Metric::new(),
        })
    }

    pub fn tcp_requests(&self) -> &AtomicU64Metric {
        &self.tcp_requests
    }

    pub fn udp_requests(&self) -> &AtomicU64Metric {
        &self.udp_requests
    }

    pub fn local_requests(&self) -> &AtomicU64Metric {
        &self.local_requests
    }

    pub fn cache_hits(&self) -> &AtomicU64Metric {
        &self.cache_hits
    }

    pub fn cache_misses(&self) -> &AtomicU64Metric {
        &self.cache_misses
    }

    pub fn doh_request_errors(&self) -> &AtomicU64Metric {
        &self.doh_request_errors
    }
}

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tcp_requests = {} udp_requests = {} local_requests = {} cache_hits = {} cache_misses = {} doh_request_errors = {}",
            self.tcp_requests().value(),
            self.udp_requests().value(),
            self.local_requests().value(),
            self.cache_hits().value(),
            self.cache_misses().value(),
            self.doh_request_errors().value(),
        )
    }
}
