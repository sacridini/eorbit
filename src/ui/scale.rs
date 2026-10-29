//! Scales shared by the charts: the colour of the sky for a Sun elevation,
//! of a quality score, and time ticks.

use chrono::{DateTime, Duration, Local, Timelike, Utc};

fn lerp_rgb(stops: &[(f64, [u8; 3])], x: f64) -> String {
    let x = x.clamp(stops[0].0, stops[stops.len() - 1].0);
    let i = stops.windows(2).position(|w| x <= w[1].0).unwrap_or(stops.len() - 2);
    let ((x0, a), (x1, b)) = (stops[i], stops[i + 1]);
    let t = if x1 > x0 { (x - x0) / (x1 - x0) } else { 0.0 };
    let c = |k: usize| (f64::from(a[k]) + (f64::from(b[k]) - f64::from(a[k])) * t).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(0), c(1), c(2))
}

/// The sky at the area for a Sun elevation: night, the three twilights,
/// sunrise, and the day getting paler as the Sun climbs.
pub fn sky(el: f64) -> String {
    const STOPS: [(f64, [u8; 3]); 10] = [
        (-90.0, [6, 12, 26]),
        (-18.0, [9, 20, 44]),
        (-12.0, [22, 32, 78]),
        (-6.0, [58, 46, 112]),
        (-1.5, [138, 72, 120]),
        (1.5, [214, 108, 74]),
        (8.0, [238, 156, 76]),
        (20.0, [243, 196, 104]),
        (45.0, [246, 222, 150]),
        (90.0, [250, 238, 196]),
    ];
    lerp_rgb(&STOPS, el)
}

/// A quality score, from slate (poor) to sunlit amber (excellent).
pub fn score(s: f64) -> String {
    const STOPS: [(f64, [u8; 3]); 4] = [(0.0, [52, 66, 86]), (35.0, [112, 106, 98]), (65.0, [214, 150, 64]), (100.0, [255, 205, 112])];
    lerp_rgb(&STOPS, s)
}

/// Tick times between `a` and `b`, about `want` of them on round hours
/// (local or UTC), each telling whether it is a midnight.
pub fn ticks(a: DateTime<Utc>, b: DateTime<Utc>, want: i64, utc: bool) -> Vec<(DateTime<Utc>, bool)> {
    let span_h = (b - a).num_hours().max(1);
    let every = [1, 2, 3, 6, 12, 24].into_iter().find(|e| span_h / e <= want).unwrap_or(24);
    let mut t = a.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(a) + Duration::hours(1);
    let mut out = Vec::new();
    while t <= b {
        let hour = if utc { t.hour() } else { t.with_timezone(&Local).hour() };
        if hour as i64 % every == 0 {
            out.push((t, hour == 0));
        }
        t += Duration::hours(1);
    }
    out
}

/// Position of `t` in `[a, b]`, in percent.
pub fn pct(t: DateTime<Utc>, (a, b): (DateTime<Utc>, DateTime<Utc>)) -> f64 {
    (t - a).num_milliseconds() as f64 / (b - a).num_milliseconds().max(1) as f64 * 100.0
}
