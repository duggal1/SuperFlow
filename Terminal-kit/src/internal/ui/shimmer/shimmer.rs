use std::sync::OnceLock;
use std::time::{Duration, Instant};

const SPINNER_CYCLE: Duration = Duration::from_millis(720);
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

fn elapsed_since_start() -> Duration {
    PROCESS_START.get_or_init(Instant::now).elapsed()
}

pub fn current_spinner_frame() -> &'static str {
    let progress = (elapsed_since_start().as_secs_f64() % SPINNER_CYCLE.as_secs_f64())
        / SPINNER_CYCLE.as_secs_f64();
    let index = ((progress * SPINNER_FRAMES.len() as f64) as usize).min(SPINNER_FRAMES.len() - 1);
    SPINNER_FRAMES[index]
}
