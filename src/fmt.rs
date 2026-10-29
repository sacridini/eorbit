//! Times and quantities as the interface shows them.

use chrono::{DateTime, Local, Utc};

pub fn time(t: DateTime<Utc>, utc: bool) -> String {
    if utc { t.format("%H:%M:%S").to_string() } else { t.with_timezone(&Local).format("%H:%M:%S").to_string() }
}

pub fn hm(t: DateTime<Utc>, utc: bool) -> String {
    if utc { t.format("%H:%M").to_string() } else { t.with_timezone(&Local).format("%H:%M").to_string() }
}

/// "Thu 30 Oct".
pub fn day(t: DateTime<Utc>, utc: bool) -> String {
    if utc { t.format("%a %-d %b").to_string() } else { t.with_timezone(&Local).format("%a %-d %b").to_string() }
}

/// "30".
pub fn day_number(t: DateTime<Utc>, utc: bool) -> String {
    if utc { t.format("%-d").to_string() } else { t.with_timezone(&Local).format("%-d").to_string() }
}

/// "UTC" or the local offset, "UTC−3".
pub fn zone(t: DateTime<Utc>, utc: bool) -> String {
    if utc {
        return "UTC".into();
    }
    let secs = t.with_timezone(&Local).offset().local_minus_utc();
    let (h, m) = (secs.abs() / 3600, secs.abs() % 3600 / 60);
    let sign = if secs < 0 { "−" } else { "+" };
    match (h, m) {
        (0, 0) => "UTC".into(),
        (h, 0) => format!("UTC{sign}{h}"),
        (h, m) => format!("UTC{sign}{h}:{m:02}"),
    }
}

/// "in 3 h 12 min", "in 2 d 4 h", "now", "12 min ago".
pub fn relative(now: DateTime<Utc>, t: DateTime<Utc>) -> String {
    let s = (t - now).num_seconds();
    let a = s.abs();
    let span = if a < 60 {
        return "now".into();
    } else if a < 3600 {
        format!("{} min", a / 60)
    } else if a < 86_400 {
        format!("{} h {} min", a / 3600, a % 3600 / 60)
    } else {
        format!("{} d {} h", a / 86_400, a % 86_400 / 3600)
    };
    if s > 0 { format!("in {span}") } else { format!("{span} ago") }
}

/// Simulation speed: "1×", "10×", "1 min/s", "6 h/s".
pub fn warp(w: f64) -> String {
    let sign = if w < 0.0 { "−" } else { "" };
    let a = w.abs();
    if a >= 3600.0 {
        format!("{sign}{} h/s", a / 3600.0)
    } else if a >= 60.0 {
        format!("{sign}{} min/s", a / 60.0)
    } else {
        format!("{sign}{a}×")
    }
}

/// How long ago `t` was, in real time: "just now", "3 h 12 min ago".
pub fn age(t: DateTime<Utc>) -> String {
    match relative(Utc::now(), t).as_str() {
        "now" => "just now".into(),
        other => other.to_string(),
    }
}
