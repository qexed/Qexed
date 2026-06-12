//! Global profiler access for Qexed server.

use std::sync::OnceLock;
use qexed_profiler::Profiler;

static PROFILER: OnceLock<Profiler> = OnceLock::new();

pub fn init(profiler: Profiler) {
    PROFILER.set(profiler).ok();
}

pub fn get() -> Option<&'static Profiler> {
    PROFILER.get()
}

#[macro_export]
macro_rules! profile_span {
    ($name:expr) => {
        {
            let _span = $crate::profiler::get().map(|p| p.span($name));
            _span
        }
    };
}
