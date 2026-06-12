//! Profiler demo — simulates server workload and generates an HTML report.
//! Run with: cargo run --example profiler_demo
//! Then open `profile_report.html` in a browser.

use qexed_profiler::Profiler;
use std::{
    thread,
    time::{Duration, Instant},
};

fn main() {
    let profiler = Profiler::new();
    println!("Profiler enabled. Simulating server load for 5 seconds...\n");
    profiler.enable();

    let start = Instant::now();
    let tick_interval = Duration::from_millis(50); // 20 TPS

    while start.elapsed() < Duration::from_secs(5) {
        let tick_start = Instant::now();

        // --- Survival tick (20 TPS sim) ---
        {
            let _span = profiler.span("tick:survival");
            burn(400); // 400µs
        }

        // --- Entity AI tick ---
        {
            let _span = profiler.span("tick:entities");
            burn(1200); // 1.2ms — usually the slowest
        }

        // --- Block update tick ---
        {
            let _span = profiler.span("tick:block_updates");
            burn(300);
        }

        // --- Chunk send (every 5 ticks) ---
        if tick_start.elapsed().as_millis() % 5 == 0 {
            let _span = profiler.span("net:chunk_send");
            burn(800);
        }

        // --- Player data autosave (every 60 ticks) ---
        if tick_start.elapsed().as_millis() % 60 == 0 {
            let _span = profiler.span("io:player_autosave");
            burn(3000); // 3ms — disk I/O simulation
        }

        // --- Redstone tick (every 2 ticks) ---
        if tick_start.elapsed().as_millis() % 2 == 0 {
            let _span = profiler.span("tick:redstone");
            burn(600);
        }

        // --- Fluid tick ---
        {
            let _span = profiler.span("tick:fluid");
            burn(100);
        }

        // Maintain 20 TPS pace
        let elapsed = tick_start.elapsed();
        if elapsed < tick_interval {
            thread::sleep(tick_interval - elapsed);
        }
    }

    profiler.disable();

    // Generate report
    let html = profiler.report_html();
    let path = "profile_report.html";
    std::fs::write(path, &html).expect("write report");
    println!("Report written to: {path}");
    println!("Open it in a browser to view the performance report.");
}

/// Burn CPU for approximately `micros` microseconds.
fn burn(micros: u64) {
    let start = Instant::now();
    while start.elapsed().as_micros() < micros as u128 {
        // Busy loop — simulates real work
        std::hint::spin_loop();
    }
}
