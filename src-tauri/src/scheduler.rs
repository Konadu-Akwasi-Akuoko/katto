//! Anacron-style scheduler (Phase 6): named jobs with a daily slot and a
//! catch-up window, evaluated on a coarse tick plus a wake nudge — a Mac that
//! slept through the slot still runs the job once on wake.

pub mod due;
pub mod runtime;
