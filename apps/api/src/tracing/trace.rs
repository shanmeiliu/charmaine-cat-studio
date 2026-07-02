use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone)]
pub struct Trace {
    inner: Arc<Mutex<RequestTrace>>,
}

pub struct RequestTrace {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub started_at: DateTime<Utc>,
    pub stages: Vec<TraceStage>,
    started_instant: Instant,
    previous_stage_at: Instant,
}

#[derive(Clone, Serialize)]
pub struct TraceStage {
    pub name: String,
    pub elapsed_micros: u64,
}

#[derive(Serialize)]
struct TraceLog {
    event: &'static str,
    request_id: String,
    method: String,
    path: String,
    status: u16,
    started_at: DateTime<Utc>,
    total_elapsed_micros: u64,
    stages: Vec<TraceStage>,
}

impl Trace {
    pub fn new(request_id: String, method: String, path: String) -> Self {
        let now = Instant::now();

        Self {
            inner: Arc::new(Mutex::new(RequestTrace {
                request_id,
                method,
                path,
                started_at: Utc::now(),
                stages: Vec::new(),
                started_instant: now,
                previous_stage_at: now,
            })),
        }
    }

    pub fn request_id(&self) -> String {
        self.inner
            .lock()
            .expect("request trace lock poisoned")
            .request_id
            .clone()
    }

    pub fn mark(&self, name: &str) {
        let now = Instant::now();
        let mut trace = self.inner.lock().expect("request trace lock poisoned");
        let elapsed_micros = micros_since(trace.previous_stage_at, now);

        trace.stages.push(TraceStage {
            name: name.to_string(),
            elapsed_micros,
        });
        trace.previous_stage_at = now;
    }

    pub fn log_completed(&self, status: u16) {
        let trace = self.inner.lock().expect("request trace lock poisoned");
        let log = TraceLog {
            event: "request_completed",
            request_id: trace.request_id.clone(),
            method: trace.method.clone(),
            path: trace.path.clone(),
            status,
            started_at: trace.started_at,
            total_elapsed_micros: micros_since(trace.started_instant, Instant::now()),
            stages: trace.stages.clone(),
        };

        match serde_json::to_string(&log) {
            Ok(line) => println!("{line}"),
            Err(err) => eprintln!("Failed to serialize request trace: {err}"),
        }
    }
}

fn micros_since(started_at: Instant, ended_at: Instant) -> u64 {
    ended_at
        .duration_since(started_at)
        .as_micros()
        .min(u128::from(u64::MAX)) as u64
}
