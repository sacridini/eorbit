//! The revisit view: how often the area will be imaged, by each satellite
//! and by all of them together, and the longest wait without an image.

use super::calendar::GOOD;
use super::scale::pct;
use super::timeline::DaysPicker;
use crate::color;
use crate::fmt;
use crate::model::Model;
use chrono::{DateTime, Duration, Utc};
use leptos::prelude::*;

struct Row {
    label: String,
    detail: String,
    color: String,
    times: Vec<DateTime<Utc>>,
    combined: bool,
}

struct Stats {
    mean: Option<Duration>,
    /// The longest wait, from now or between two images.
    longest: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

fn stats(times: &[DateTime<Utc>], now: DateTime<Utc>) -> Stats {
    let mean = (times.len() >= 2).then(|| (times[times.len() - 1] - times[0]) / (times.len() as i32 - 1));
    let mut longest: Option<(DateTime<Utc>, DateTime<Utc>)> = None;
    let mut prev = now;
    for &t in times {
        if longest.is_none_or(|(a, b)| t - prev > b - a) {
            longest = Some((prev, t));
        }
        prev = t;
    }
    Stats { mean, longest }
}

#[component]
pub fn Revisit() -> impl IntoView {
    let m = expect_context::<Model>();

    let table = move || {
        let (Some(w), Some(fc)) = (m.window.get(), m.forecast.get()) else {
            return view! { <p class="hint">"Forecasting…"</p> }.into_any();
        };
        let now = m.now.get();
        let sats = m.sats.get();
        let unusable = m.show_unusable.get();
        let span = (now, w.1);

        let upcoming: Vec<_> = fc
            .acquisitions
            .iter()
            .filter(|a| a.time >= now && sats.get(a.sat).is_some_and(|s| unusable || a.usable(s.info.kind)))
            .collect();
        let mut rows = vec![
            Row {
                label: "All satellites".into(),
                detail: "any opportunity".into(),
                color: "#e9f0f8".into(),
                times: upcoming.iter().map(|a| a.time).collect(),
                combined: true,
            },
            Row {
                label: "Good images".into(),
                detail: format!("score ≥ {GOOD:.0}, within the cloud forecast"),
                color: "#ffb547".into(),
                times: upcoming.iter().filter(|a| untrack(|| m.score(a)) >= GOOD).map(|a| a.time).collect(),
                combined: true,
            },
        ];
        let mut per_sat: Vec<Row> = sats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let times: Vec<_> = upcoming.iter().filter(|a| a.sat == i).map(|a| a.time).collect();
                (!times.is_empty()).then(|| Row {
                    label: s.info.name.to_string(),
                    detail: s.info.sensor.to_string(),
                    color: color::css(s.info.color),
                    times,
                    combined: false,
                })
            })
            .collect();
        per_sat.sort_by_key(|r| stats(&r.times, now).mean.unwrap_or(Duration::MAX));
        rows.extend(per_sat);

        let body = rows
            .into_iter()
            .map(|row| {
                let s = stats(&row.times, now);
                let ticks = row
                    .times
                    .iter()
                    .map(|&t| view! { <i class="rv-tick" style=format!("left: {:.3}%", pct(t, span))></i> })
                    .collect_view();
                let gap = s.longest.map(|(a, b)| {
                    let (x0, x1) = (pct(a, span), pct(b, span));
                    view! { <div class="rv-gap" style=format!("left: {x0:.3}%; width: {:.3}%", x1 - x0)></div> }
                });
                let next = row.times.first().map(|&t| fmt::relative(now, t)).unwrap_or_else(|| "none".into());
                view! {
                    <div class="rv-row" class:combined=row.combined style=format!("--sat: {}", row.color)>
                        <span class="rv-name">
                            <strong>{row.label}</strong>
                            <span class="muted">{row.detail}</span>
                        </span>
                        <span class="rv-num">{row.times.len()}</span>
                        <span class="rv-num">{s.mean.map_or_else(|| "–".into(), fmt::span)}</span>
                        <span class="rv-num">{s.longest.map_or_else(|| "–".into(), |(a, b)| fmt::span(b - a))}</span>
                        <span class="rv-num">{next}</span>
                        <div class="rv-strip">{gap}{ticks}</div>
                    </div>
                }
            })
            .collect_view();

        view! {
            <div class="rv">
                <div class="rv-row rv-head muted">
                    <span class="rv-name">"Satellite"</span>
                    <span class="rv-num">"Images"</span>
                    <span class="rv-num">"Mean revisit"</span>
                    <span class="rv-num">"Longest wait"</span>
                    <span class="rv-num">"Next"</span>
                    <span class="rv-strip-head">
                        {move || format!("From now to {}", fmt::day(w.1, m.utc.get()))}
                    </span>
                </div>
                {body}
            </div>
        }
        .into_any()
    };

    view! {
        <section class="board panel revisit">
            <div class="board-head">
                <h2>"Revisit"</h2>
                <DaysPicker/>
                <p class="legend muted">
                    "How often your area is imaged over the forecast. The shaded span is the longest wait without an image; longer forecasts steady the averages."
                </p>
            </div>
            <div class="board-body">{table}</div>
        </section>
    }
}
