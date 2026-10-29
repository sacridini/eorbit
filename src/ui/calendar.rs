//! The calendar view: days by hours, each hour tinted by the sky over the
//! area and holding its best imaging opportunity, so that the day with the
//! best chance of a clear image stands out.

use super::scale;
use super::timeline::DaysPicker;
use crate::color;
use crate::fmt;
use crate::model::{self, Model};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use leptos::prelude::*;
use orbitui::predict::Acquisition;
use std::collections::BTreeMap;

/// Scores from which an opportunity counts as good.
pub const GOOD: f64 = 50.0;

fn date_hour(t: DateTime<Utc>, utc: bool) -> (NaiveDate, u32) {
    if utc { (t.date_naive(), t.hour()) } else { (t.with_timezone(&Local).date_naive(), t.with_timezone(&Local).hour()) }
}

fn day_start(d: NaiveDate, utc: bool) -> Option<DateTime<Utc>> {
    let midnight = d.and_hms_opt(0, 0, 0)?;
    if utc { Some(midnight.and_utc()) } else { Local.from_local_datetime(&midnight).earliest().map(|t| t.with_timezone(&Utc)) }
}

#[component]
pub fn Calendar() -> impl IntoView {
    let m = expect_context::<Model>();

    let grid = move || {
        let (Some(w), Some(fc)) = (m.window.get(), m.forecast.get()) else {
            return view! { <p class="hint">"Forecasting…"</p> }.into_any();
        };
        let utc = m.utc.get();
        let obs = m.observer.get();
        let sats = m.sats.get();
        let unusable = m.show_unusable.get();
        let now = m.now.get_untracked();

        // Opportunities by day and hour, with their scores.
        let mut cells: BTreeMap<(NaiveDate, u32), Vec<(f64, Acquisition)>> = BTreeMap::new();
        for a in fc.acquisitions.iter().filter(|a| a.time >= now - Duration::hours(1)) {
            let Some(s) = sats.get(a.sat) else { continue };
            if !unusable && !a.usable(s.info.kind) {
                continue;
            }
            let score = untrack(|| m.score(a));
            cells.entry(date_hour(a.time, utc)).or_default().push((score, a.clone()));
        }
        for list in cells.values_mut() {
            list.sort_by(|a, b| b.0.total_cmp(&a.0));
        }

        let (first, last) = (date_hour(w.0.max(now), utc).0, date_hour(w.1, utc).0);
        let days: Vec<NaiveDate> = first.iter_days().take_while(|d| *d <= last).collect();

        // The best opportunity of the forecast.
        let best = cells.values().filter_map(|l| l.first()).max_by(|a, b| a.0.total_cmp(&b.0)).cloned();
        let banner = best.map(|(score, a)| {
            let name = sats.get(a.sat).map_or("", |s| s.info.name);
            let time = a.time;
            view! {
                <button class="banner" on:click=move |_| m.open_event(a.clone())>
                    {format!("Best chance: {name}, {} at {}, score {score:.0}", fmt::day(time, utc), fmt::hm(time, utc))}
                </button>
            }
        });

        let header = (0..24)
            .map(|h| view! { <span class="cal-hour">{if h % 3 == 0 { format!("{h:02}") } else { String::new() }}</span> })
            .collect_view();

        let rows = days
            .into_iter()
            .map(|d| {
                let start = day_start(d, utc);
                let label = start.map(|t| fmt::day(t, utc)).unwrap_or_default();
                let mut good = 0;
                let mut best_of_day: Option<(f64, Acquisition)> = None;
                let hours = (0..24u32)
                    .map(|h| {
                        let mid = start.map(|t| t + Duration::minutes(i64::from(h) * 60 + 30));
                        let sky = mid.map(|t| scale::sky(model::sun_elevation(&obs, t))).unwrap_or_default();
                        let list = cells.get(&(d, h)).cloned().unwrap_or_default();
                        good += list.iter().filter(|(s, _)| *s >= GOOD).count();
                        if let Some(top) = list.first()
                            && best_of_day.as_ref().is_none_or(|b| top.0 > b.0)
                        {
                            best_of_day = Some(top.clone());
                        }
                        let chip = list.first().cloned().map(|(score, a)| {
                            let title = list
                                .iter()
                                .filter_map(|(s, a)| Some(format!("{} at {}: score {s:.0}", sats.get(a.sat)?.info.name, fmt::hm(a.time, utc))))
                                .collect::<Vec<_>>()
                                .join("\n");
                            let sat_color = sats.get(a.sat).map(|s| color::css(s.info.color)).unwrap_or_default();
                            let more = list.len() > 1;
                            view! {
                                <button
                                    class="cal-chip"
                                    style=format!("background: {}; --sat: {sat_color}", scale::score(score))
                                    title=title
                                    on:click=move |_| m.open_event(a.clone())
                                >
                                    {format!("{score:.0}")}
                                    {more.then(|| view! { <i class="more"></i> })}
                                </button>
                            }
                        });
                        view! { <div class="cal-cell" style=format!("--sky: {sky}")>{chip}</div> }
                    })
                    .collect_view();
                let summary = best_of_day.map(|(score, a)| {
                    let name = sats.get(a.sat).map_or("", |s| s.info.short);
                    let time = a.time;
                    view! {
                        <button class="cal-best" on:click=move |_| m.open_event(a.clone())>
                            {format!("{name} {} · {score:.0}", fmt::hm(time, utc))}
                        </button>
                    }
                });
                view! {
                    <div class="cal-row">
                        <span class="cal-day">{label}</span>
                        <div class="cal-hours">{hours}</div>
                        <span class="cal-good" class:none=good == 0>
                            {if good == 1 { "1 good".to_string() } else { format!("{good} good") }}
                        </span>
                        <span class="cal-sum">{summary}</span>
                    </div>
                }
            })
            .collect_view();

        view! {
            {banner}
            <div class="cal">
                <div class="cal-row cal-head">
                    <span class="cal-day"></span>
                    <div class="cal-hours">{header}</div>
                    <span class="cal-good muted">"Score ≥ 50"</span>
                    <span class="cal-sum muted">"Best of the day"</span>
                </div>
                {rows}
            </div>
        }
        .into_any()
    };

    view! {
        <section class="board panel calendar">
            <div class="board-head">
                <h2>"Calendar"</h2>
                <DaysPicker/>
                <div class="scale-legend" aria-label="Score scale">
                    <span class="muted">"Score"</span>
                    {[10.0, 30.0, 50.0, 70.0, 90.0]
                        .into_iter()
                        .map(|s| view! { <i style=format!("background: {}", scale::score(s))>{format!("{s:.0}")}</i> })
                        .collect_view()}
                </div>
                <p class="legend muted">
                    {move || format!("Hours in {}, tinted by the sky over the area; each holds its best opportunity", if m.utc.get() { "UTC" } else { "local time" })}
                </p>
            </div>
            <div class="board-body">{grid}</div>
        </section>
    }
}
