use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use enum_iterator::IntoEnumIterator;

pub trait Metric: Display {
    fn name(&self) -> &str;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, IntoEnumIterator)]
pub enum CounterMetricType {
    TCPRequests,
    UDPRequests,
    LocalRequests,
    CacheHits,
    CacheMisses,
    DOHRequestErrors,
}

impl CounterMetricType {
    fn name(&self) -> &'static str {
        match self {
            CounterMetricType::TCPRequests => "tcp_requests",
            CounterMetricType::UDPRequests => "udp_requests",
            CounterMetricType::LocalRequests => "local_requests",
            CounterMetricType::CacheHits => "cache_hits",
            CounterMetricType::CacheMisses => "cache_misses",
            CounterMetricType::DOHRequestErrors => "doh_request_errors",
        }
    }
}

pub struct CounterMetric {
    value: AtomicU64,
    counter_metric_type: CounterMetricType,
}

impl CounterMetric {
    fn new(counter_metric_type: CounterMetricType) -> Self {
        CounterMetric {
            value: AtomicU64::new(0),
            counter_metric_type,
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
        self.counter_metric_type.name()
    }
}

impl Display for CounterMetric {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.name(), self.value())
    }
}

pub struct Metrics {
    counter_metrics: Vec<CounterMetric>,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        let mut counter_metrics = Vec::with_capacity(CounterMetricType::VARIANT_COUNT);

        for counter_metric_type in CounterMetricType::into_enum_iter() {
            counter_metrics.push(CounterMetric::new(counter_metric_type));
        }

        Arc::new(Metrics { counter_metrics })
    }

    pub fn counter_metric(&self, counter_metric_type: CounterMetricType) -> &CounterMetric {
        &self.counter_metrics[counter_metric_type as usize]
    }

    pub fn all_metrics(&self) -> Vec<&dyn Metric> {
        CounterMetricType::into_enum_iter()
            .map(|counter_metric_type| self.counter_metric(counter_metric_type) as &dyn Metric)
            .collect()
    }

    pub fn all_metrics_string(&self) -> String {
        self.all_metrics()
            .iter()
            .map(|&metric| metric.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }
}
