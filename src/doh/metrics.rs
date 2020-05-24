use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub trait Metric: Display {
    fn name(&self) -> &str;
}

pub struct CounterMetric {
    value: AtomicU64,
    name: String,
}

impl CounterMetric {
    fn new(name: &str) -> Self {
        CounterMetric {
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
}

impl Metric for CounterMetric {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Display for CounterMetric {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.name(), self.value())
    }
}

pub struct Metrics {
    tcp_requests: CounterMetric,
    udp_requests: CounterMetric,
    local_requests: CounterMetric,
    cache_hits: CounterMetric,
    cache_misses: CounterMetric,
    doh_request_errors: CounterMetric,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            tcp_requests: CounterMetric::new("tcp_requests"),
            udp_requests: CounterMetric::new("udp_requests"),
            local_requests: CounterMetric::new("local_requests"),
            cache_hits: CounterMetric::new("cache_hits"),
            cache_misses: CounterMetric::new("cache_misses"),
            doh_request_errors: CounterMetric::new("doh_request_errors"),
        })
    }

    pub fn tcp_requests(&self) -> &CounterMetric {
        &self.tcp_requests
    }

    pub fn udp_requests(&self) -> &CounterMetric {
        &self.udp_requests
    }

    pub fn local_requests(&self) -> &CounterMetric {
        &self.local_requests
    }

    pub fn cache_hits(&self) -> &CounterMetric {
        &self.cache_hits
    }

    pub fn cache_misses(&self) -> &CounterMetric {
        &self.cache_misses
    }

    pub fn doh_request_errors(&self) -> &CounterMetric {
        &self.doh_request_errors
    }

    pub fn all_metrics(&self) -> Vec<&dyn Metric> {
        vec![
            &self.tcp_requests,
            &self.udp_requests,
            &self.local_requests,
            &self.cache_hits,
            &self.cache_misses,
            &self.doh_request_errors
        ]
    }

    pub fn all_metrics_string(&self) -> String {
        self.all_metrics()
            .iter()
            .map(|&metric| metric.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }
}
