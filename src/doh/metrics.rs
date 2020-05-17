use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct AtomicU64Metric {
    value: AtomicU64,
    name: String,
}

impl AtomicU64Metric {
    fn new(name: &str) -> Self {
        AtomicU64Metric {
            value: AtomicU64::new(0),
            name: name.to_owned(),
        }
    }

    pub fn increment_value(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}

impl fmt::Display for AtomicU64Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.name(), self.value())
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
            tcp_requests: AtomicU64Metric::new("tcp_requests"),
            udp_requests: AtomicU64Metric::new("udp_requests"),
            local_requests: AtomicU64Metric::new("local_requests"),
            cache_hits: AtomicU64Metric::new("cache_hits"),
            cache_misses: AtomicU64Metric::new("cache_misses"),
            doh_request_errors: AtomicU64Metric::new("doh_request_errors"),
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

    fn all_metrics(&self) -> Vec<&AtomicU64Metric> {
        vec![
            &self.tcp_requests,
            &self.udp_requests,
            &self.local_requests,
            &self.cache_hits,
            &self.cache_misses,
            &self.doh_request_errors,
        ]
    }
}

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let all_metrics = self.all_metrics();
        let mut first = true;

        for metric in all_metrics {
            if !first {
                write!(f, " ")?;
            }
            metric.fmt(f)?;
            first = false;
        }
        Ok(())
    }
}
