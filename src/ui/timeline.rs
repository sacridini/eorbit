//! The time strip: the simulated clock, and every opportunity of the
//! forecast laid out in time against night and cloud cover. Dragging along
//! it moves the clock.

use crate::color;
use crate::fmt;
use crate::model::{self, Model};
use chrono::{DateTime, Duration, Local, Timelike, Utc};
use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

const WARPS: [f64; 6] = [1.0, 10.0, 60.0, 600.0, 3600.0, 21600.0];
pub const DAYS: [f64; 4] = [3.0, 7.0, 14.0, 30.0];

fn pct(t: DateTime<Utc>, (a, b): (DateTime<Utc>, DateTime<Utc>)) -> f64 {
    (t - a).num_seconds() as f64 / (b - a).num_seconds().max(1) as f64 * 100.0
}

/// Whole hours in `[a, b]` that are multiples of `every` (local or UTC).
fn hours(a: DateTime<Utc>, b: DateTime<Utc>, every: u32, utc: bool) -> Vec<DateTime<Utc>> {
    let mut t = a.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(a) + Duration::hours(1);
    let mut out = Vec::new();
    while t <= b {
        let hour = if utc { t.hour() } else { t.with_timezone(&Local).hour() };
        if hour % every == 0 {
            out.push(t);
        }
        t += Duration::hours(1);
    }
    out
}

/// Spans of the window when the Sun is below the AOI's horizon.
fn nights(m: Model, (a, b): (DateTime<Utc>, DateTime<Utc>)) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let obs = m.observer.get();
    let mut out = Vec::new();
    let mut start = None;
    let mut t = a;
    while t <= b {
        let dark = model::sun_elevation(&obs, t) < 0.0;
        match (dark, start) {
            (true, None) => start = Some(t),
            (false, Some(s)) => {
                out.push((s, t));
                start = None;
            }
            _ => {}
        }
        t += Duration::minutes(10);
    }
    if let Some(s) = start {
        out.push((s, b));
    }
    out
}

#[component]
pub fn Timeline() -> impl IntoView {
    let m = expect_context::<Model>();
    let axis = NodeRef::<html::Div>::new();
    let dragging = RwSignal::new(false);

    let seek = move |x: f64| {
        let (Some(track), Some(w)) = (axis.get_untracked(), m.window.get_untracked()) else { return };
        let r = track.get_bounding_client_rect();
        let f = ((x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
        let t = w.0 + Duration::milliseconds(((w.1 - w.0).num_milliseconds() as f64 * f) as i64);
        m.set_time(t);
    };

    let day_marks = move || {
        let Some(w) = m.window.get() else { return Vec::new().collect_view() };
        let utc = m.utc.get();
        let every = match (w.1 - w.0).num_days() {
            0..=4 => 6,
            5..=8 => 12,
            _ => 24,
        };
        hours(w.0, w.1, every, utc)
            .into_iter()
            .map(|t| {
                let midnight = if utc { t.hour() == 0 } else { t.with_timezone(&Local).hour() == 0 };
                let label = if midnight { fmt::day(t, utc) } else { fmt::hm(t, utc) };
                let short = if midnight { fmt::day_number(t, utc) } else { String::new() };
                view! {
                    <span class="tick" class:day=midnight style=format!("left: {:.3}%", pct(t, w))>
                        <span class="long">{label}</span>
                        <span class="short">{short}</span>
                    </span>
                }
            })
            .collect_view()
    };

    let night_bands = move || {
        let Some(w) = m.window.get() else { return Vec::new().collect_view() };
        nights(m, w)
            .into_iter()
            .map(|(a, b)| {
                view! { <div class="night" style=format!("left: {:.3}%; width: {:.3}%", pct(a, w), pct(b, w) - pct(a, w))></div> }
            })
            .collect_view()
    };

    let cloud_area = move || {
        let w = m.window.get()?;
        let c = m.clouds.get()?;
        let pts: Vec<(f64, f64)> = (0..c.hourly.len())
            .filter_map(|h| {
                let t = c.start + Duration::hours(h as i64);
                let v = c.hourly[h];
                (t >= w.0 - Duration::hours(1) && t <= w.1 + Duration::hours(1) && v.is_finite()).then(|| (pct(t, w) * 10.0, 100.0 - v))
            })
            .collect();
        let (first, last) = (pts.first()?.0, pts.last()?.0);
        let line: String = pts.iter().map(|(x, y)| format!(" L{x:.1} {y:.1}")).collect();
        let area = format!("M{first:.1} 100{line} L{last:.1} 100 Z");
        let stroke = format!("M{}", &line[2..]);
        Some(view! {
            <svg class="cloud" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
                <path class="cloud-fill" d=area/>
                <path class="cloud-line" d=stroke/>
            </svg>
        })
    };

    let rows = move || {
        let (Some(w), Some(fc)) = (m.window.get(), m.forecast.get()) else { return Vec::new().collect_view() };
        let sats = m.sats.get();
        let selected = m.selected_index();
        let utc = m.utc.get();
        sats.iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let events: Vec<_> = fc.acquisitions.iter().filter(|a| a.sat == i).cloned().collect();
                if events.is_empty() {
                    return None;
                }
                let info = s.info;
                let marks = events
                    .into_iter()
                    .map(|a| {
                        let usable = a.usable(info.kind);
                        let title = format!("{}: {}, {} · {:.0}° off-nadir", info.name, fmt::day(a.time, utc), fmt::hm(a.time, utc), a.off_nadir);
                        let left = pct(a.time, w);
                        view! {
                            <button
                                class="mark"
                                class:unusable=!usable
                                style=format!("left: {left:.3}%")
                                title=title.clone()
                                aria-label=title
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:click=move |_| m.open_event(a.clone())
                            ></button>
                        }
                    })
                    .collect_view();
                Some(view! {
                    <div class="tl-row" class:sel=selected == Some(i) style=format!("--sat: {}", color::css(info.color))>
                        <button class="tl-label" on:click=move |_| m.select(info.norad)>{info.short}</button>
                        <div class="tl-track">{marks}</div>
                    </div>
                })
            })
            .collect_view()
    };

    let cursor = move || {
        let w = m.window.get()?;
        let left = pct(m.now.get(), w);
        (0.0..=100.0).contains(&left).then(|| view! { <div class="cursor" style=format!("left: {left:.3}%")></div> })
    };

    view! {
        <section class="timeline panel">
            <Transport/>
            <div class="tl">
                <div
                    class="tl-content"
                    class:dragging=dragging
                    on:pointerdown=move |ev| {
                        if let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                            el.set_pointer_capture(ev.pointer_id()).ok();
                        }
                        dragging.set(true);
                        seek(ev.client_x() as f64);
                    }
                    on:pointermove=move |ev| {
                        if dragging.get_untracked() {
                            seek(ev.client_x() as f64);
                        }
                    }
                    on:pointerup=move |_| dragging.set(false)
                    on:pointercancel=move |_| dragging.set(false)
                >
                    <div class="tl-layer" aria-hidden="true">{night_bands}</div>
                    <div class="tl-row tl-axis">
                        <span class="tl-label"></span>
                        <div class="tl-track" node_ref=axis>{day_marks}</div>
                    </div>
                    <div class="tl-row tl-clouds">
                        <span class="tl-label muted">"Clouds"</span>
                        <div class="tl-track">{cloud_area}</div>
                    </div>
                    <div class="tl-rows">{rows}</div>
                    <div class="tl-layer tl-top" aria-hidden="true">{cursor}</div>
                </div>
            </div>
        </section>
    }
}

#[component]
fn Transport() -> impl IntoView {
    let m = expect_context::<Model>();
    let utc = m.utc;
    let live = move || {
        m.now.track();
        m.clock.with(|c| c.is_live())
    };
    let paused = move || m.clock.with(|c| c.paused);
    view! {
        <div class="transport">
            <button class="readout" title="Switch between local time and UTC" on:click=move |_| utc.update(|u| *u = !*u)>
                <span class="big">{move || fmt::time(m.now.get(), utc.get())}</span>
                <span class="muted">{move || format!("{}, {}", fmt::day(m.now.get(), utc.get()), fmt::zone(m.now.get(), utc.get()))}</span>
            </button>
            <div class="controls">
                <button
                    class="play"
                    title=move || if paused() { "Play" } else { "Pause" }
                    aria-label=move || if paused() { "Play" } else { "Pause" }
                    on:click=move |_| m.clock.update(|c| c.toggle())
                >
                    {move || {
                        if paused() {
                            view! { <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg> }.into_any()
                        } else {
                            view! { <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h4v14H7zM13 5h4v14h-4z"/></svg> }.into_any()
                        }
                    }}
                </button>
                <div class="segmented" role="group" aria-label="Speed">
                    {WARPS
                        .into_iter()
                        .map(|w| {
                            view! {
                                <button
                                    class:on=move || m.clock.with(|c| c.warp == w && !c.paused)
                                    on:click=move |_| m.clock.update(|c| c.set_warp(w))
                                >
                                    {fmt::warp(w)}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                <button
                    class="chip live"
                    class:on=live
                    title="Back to the present, in real time"
                    on:click=move |_| {
                        m.set_time(Utc::now());
                        m.clock.update(|c| c.set_warp(1.0));
                    }
                >
                    "Now"
                </button>
            </div>
            <div class="horizon">
                <span class="muted">"Forecast"</span>
                <DaysPicker/>
            </div>
        </div>
    }
}


/// The length of the forecast.
#[component]
pub fn DaysPicker() -> impl IntoView {
    let m = expect_context::<Model>();
    view! {
        <div class="segmented" role="group" aria-label="Forecast length">
            {DAYS
                .into_iter()
                .map(|d| {
                    view! {
                        <button
                            class:on=move || m.days.get() == d
                            title=format!("Forecast the next {d:.0} days")
                            on:click=move |_| {
                                m.days.set(d);
                                m.request_forecast();
                            }
                        >
                            {format!("{d:.0} d")}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
