//! Opt-in, sampled diagnostic stage timings; never includes keys, bodies or URLs.
use std::{io::Write, sync::OnceLock, time::Instant};
pub(crate) struct Trace {
    name: &'static str,
    at: Option<Instant>,
    marks: Vec<(&'static str, f64)>,
    counts: Vec<(&'static str, u64)>,
}
impl Trace {
    pub fn new(name: &'static str) -> Self {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        let enabled =
            *ENABLED.get_or_init(|| std::env::var("MANGA_BENCH_PROFILE").is_ok_and(|s| s == "1"));
        static SAMPLE_EVERY: OnceLock<u16> = OnceLock::new();
        let every = *SAMPLE_EVERY.get_or_init(|| {
            std::env::var("MANGA_BENCH_PROFILE_EVERY")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|value| (1..=256).contains(value))
                .unwrap_or(16)
        });
        let sampled = enabled && rand::random::<u16>().is_multiple_of(every);
        Self {
            name,
            at: sampled.then(Instant::now),
            marks: Vec::new(),
            counts: Vec::new(),
        }
    }
    pub fn count(&mut self, name: &'static str, value: u64) {
        if self.at.is_some() {
            self.counts.push((name, value));
        }
    }
    pub fn mark(&mut self, name: &'static str) {
        if let Some(at) = self.at {
            self.marks.push((name, at.elapsed().as_secs_f64() * 1000.));
            self.at = Some(Instant::now());
        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        if self.at.is_some() {
            self.mark("remaining");
            let _ = writeln!(
                std::io::stderr().lock(),
                "LIBRARY_PERF {}",
                serde_json::json!({"operation":self.name,"stages_ms":self.marks,"counts":self.counts,"at_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()})
            );
        }
    }
}
