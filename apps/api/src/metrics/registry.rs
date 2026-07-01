use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

#[derive(Default)]
struct EndpointLatency {
    requests: AtomicU64,
    total_micros: AtomicU64,
    max_micros: AtomicU64,
}

impl EndpointLatency {
    fn record(&self, elapsed_micros: u64) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.total_micros
            .fetch_add(elapsed_micros, Ordering::Relaxed);
        self.record_max(elapsed_micros);
    }

    fn record_max(&self, elapsed_micros: u64) {
        let mut current = self.max_micros.load(Ordering::Relaxed);

        while elapsed_micros > current {
            match self.max_micros.compare_exchange_weak(
                current,
                elapsed_micros,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    fn snapshot(&self) -> EndpointLatencySnapshot {
        let requests = self.requests.load(Ordering::Relaxed);
        let total_micros = self.total_micros.load(Ordering::Relaxed);
        let average_micros = if requests == 0 {
            0
        } else {
            total_micros / requests
        };

        EndpointLatencySnapshot {
            requests,
            total_micros,
            average_micros,
            max_micros: self.max_micros.load(Ordering::Relaxed),
        }
    }
}

pub struct MetricsRegistry {
    server_started_at: Instant,
    requests_total: AtomicU64,
    requests_per_endpoint: RwLock<HashMap<String, Arc<AtomicU64>>>,
    requests_per_method: RwLock<HashMap<String, Arc<AtomicU64>>>,
    errors_total: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    latency_per_endpoint: RwLock<HashMap<String, Arc<EndpointLatency>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            server_started_at: Instant::now(),
            requests_total: AtomicU64::new(0),
            requests_per_endpoint: RwLock::new(HashMap::new()),
            requests_per_method: RwLock::new(HashMap::new()),
            errors_total: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            latency_per_endpoint: RwLock::new(HashMap::new()),
        }
    }

    pub fn record_request(
        &self,
        method: &str,
        endpoint: &str,
        status_code: u16,
        elapsed_micros: u64,
    ) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.increment_counter(&self.requests_per_endpoint, endpoint);
        self.increment_counter(&self.requests_per_method, method);
        self.record_latency(endpoint, elapsed_micros);

        if status_code >= 400 {
            self.errors_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            uptime_seconds: self.server_started_at.elapsed().as_secs(),
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_per_endpoint: self.snapshot_counters(&self.requests_per_endpoint),
            requests_per_method: self.snapshot_counters(&self.requests_per_method),
            errors_total: self.errors_total.load(Ordering::Relaxed),
            cache: CacheMetricsSnapshot {
                hits: self.cache_hits.load(Ordering::Relaxed),
                misses: self.cache_misses.load(Ordering::Relaxed),
            },
            latency_per_endpoint: self.snapshot_latency(),
        }
    }

    fn increment_counter(&self, counters: &RwLock<HashMap<String, Arc<AtomicU64>>>, key: &str) {
        let counter = {
            let mut counters = counters.write().expect("metrics lock poisoned");

            counters
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(0)))
                .clone()
        };

        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn record_latency(&self, endpoint: &str, elapsed_micros: u64) {
        let latency = {
            let mut latencies = self
                .latency_per_endpoint
                .write()
                .expect("metrics lock poisoned");

            latencies
                .entry(endpoint.to_string())
                .or_insert_with(|| Arc::new(EndpointLatency::default()))
                .clone()
        };

        latency.record(elapsed_micros);
    }

    fn snapshot_counters(
        &self,
        counters: &RwLock<HashMap<String, Arc<AtomicU64>>>,
    ) -> HashMap<String, u64> {
        counters
            .read()
            .expect("metrics lock poisoned")
            .iter()
            .map(|(key, counter)| (key.clone(), counter.load(Ordering::Relaxed)))
            .collect()
    }

    fn snapshot_latency(&self) -> HashMap<String, EndpointLatencySnapshot> {
        self.latency_per_endpoint
            .read()
            .expect("metrics lock poisoned")
            .iter()
            .map(|(endpoint, latency)| (endpoint.clone(), latency.snapshot()))
            .collect()
    }
}

#[derive(Serialize)]
pub struct MetricsSnapshot {
    pub uptime_seconds: u64,
    pub requests_total: u64,
    pub requests_per_endpoint: HashMap<String, u64>,
    pub requests_per_method: HashMap<String, u64>,
    pub errors_total: u64,
    pub cache: CacheMetricsSnapshot,
    pub latency_per_endpoint: HashMap<String, EndpointLatencySnapshot>,
}

#[derive(Serialize)]
pub struct CacheMetricsSnapshot {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Serialize)]
pub struct EndpointLatencySnapshot {
    pub requests: u64,
    pub total_micros: u64,
    pub average_micros: u64,
    pub max_micros: u64,
}
