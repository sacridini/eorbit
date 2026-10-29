//! The timeline view: the forecast laid out in time at any zoom, with the
//! Sun and the clouds over the area, every satellite's opportunities and
//! horizon passes, and a crosshair that reads them all at one instant.
//! Wheel to zoom, drag to pan, click to move the clock.

use super::scale::{self, pct};
use super::timeline::DaysPicker;
use crate::color;
use crate::fmt;
use crate::model::{self, Model};
use chrono::{DateTime, Duration, Utc};
use leptos::html;
use leptos::prelude::*;
use orbitui::plan::Status;
use wasm_bindgen::JsCast;

const SPANS: [(&str, i64); 5] = [("6 h", 6), ("1 d", 24), ("3 d", 72), ("7 d", 168), ("14 d", 336)];
const SUN_SAMPLES: i32 = 240;
const MIN_SPAN: Duration = Duration::hours(2);

type Range = (DateTime<Utc>, DateTime<Utc>);

/// `r` moved and shrunk to fit inside `w`.
fn fit(r: Range, w: Range) -> Range {
    let span = (r.1 - r.0).clamp(MIN_SPAN, w.1 - w.0);
    let mut a = r.0.max(w.0);
    if a + span > w.1 {
        a = w.1 - span;
    }
    (a, a + span)
}

fn scaled(d: Duration, f: f64) -> Duration {
    Duration::milliseconds((d.num_milliseconds() as f64 * f) as i64)
}

#[component]
pub fn Chart() -> impl IntoView {
    let m = expect_context::<Model>();
    let range = RwSignal::new(None::<Range>);
    let shown = Memo::new(move |_| {
        let w = m.window.get()?;
        let r = range.get().unwrap_or_else(|| {
            let now = m.now.get_untracked();
            (now - Duration::hours(2), now + Duration::hours(22))
        });
        Some(fit(r, w))
    });
    let hover = RwSignal::new(None::<DateTime<Utc>>);
    let track = NodeRef::<html::Div>::new();
    let drag = StoredValue::new(None::<(f64, Range, bool)>);

    let time_at = move |x: f64| {
        let (el, r) = (track.get_untracked()?, shown.get_untracked()?);
        let rect = el.get_bounding_client_rect();
        let f = (x - rect.left()) / rect.width().max(1.0);
        (0.0..=1.0).contains(&f).then(|| r.0 + scaled(r.1 - r.0, f))
    };
    let set_span = move |hours: i64| {
        let Some(r) = shown.get_untracked() else { return };
        let now = m.now.get_untracked();
        let center = if now >= r.0 && now <= r.1 { now } else { r.0 + (r.1 - r.0) / 2 };
        let half = Duration::hours(hours) / 2;
        range.set(Some((center - half, center + half)));
    };
    let pan = move |f: f64| {
        if let Some(r) = shown.get_untracked() {
            let d = scaled(r.1 - r.0, f);
            range.set(Some((r.0 + d, r.1 + d)));
        }
    };

    // ---- Bands ----

    let sun_samples = Memo::new(move |_| {
        let r = shown.get()?;
        let obs = m.observer.get();
        Some(
            (0..=SUN_SAMPLES)
                .map(|k| {
                    let t = r.0 + (r.1 - r.0) * k / SUN_SAMPLES;
                    (f64::from(k) / f64::from(SUN_SAMPLES) * 100.0, model::sun_elevation(&obs, t))
                })
                .collect::<Vec<_>>(),
        )
    });

    let sun_band = move || {
        let pts = sun_samples.get()?;
        let stops = pts
            .iter()
            .map(|&(x, el)| view! { <stop offset=format!("{x:.2}%") stop-color=scale::sky(el)/> })
            .collect_view();
        let curve: String = pts
            .iter()
            .enumerate()
            .map(|(i, &(x, el))| format!("{}{:.1} {:.1}", if i == 0 { "M" } else { " L" }, x * 10.0, 50.0 - el / 90.0 * 46.0))
            .collect();
        Some(view! {
            <svg class="sun-band" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
                <defs>
                    <linearGradient id="sky-gradient" x1="0" x2="1" y1="0" y2="0">{stops}</linearGradient>
                </defs>
                <rect width="1000" height="100" fill="url(#sky-gradient)"/>
                <line class="horizon" x1="0" y1="50" x2="1000" y2="50"/>
                <path class="sun-curve" d=curve/>
            </svg>
        })
    };

    let night_bands = move || {
        let pts = sun_samples.get().unwrap_or_default();
        let mut bands = Vec::new();
        let mut start = None;
        for &(x, el) in &pts {
            match (el < 0.0, start) {
                (true, None) => start = Some(x),
                (false, Some(s)) => {
                    bands.push((s, x));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            bands.push((s, 100.0));
        }
        bands
            .into_iter()
            .map(|(a, b)| view! { <div class="night" style=format!("left: {a:.2}%; width: {:.2}%", b - a)></div> })
            .collect_view()
    };

    let cloud_total = move || {
        let r = shown.get()?;
        m.clouds.with(|c| {
            let c = c.as_ref()?;
            let pts: Vec<(f64, f64)> = c
                .hourly
                .iter()
                .enumerate()
                .filter_map(|(h, &v)| {
                    let t = c.start + Duration::hours(h as i64);
                    (t >= r.0 - Duration::hours(1) && t <= r.1 + Duration::hours(1) && v.is_finite())
                        .then(|| (pct(t, r) * 10.0, 100.0 - v))
                })
                .collect();
            let (first, last) = (pts.first()?.0, pts.last()?.0);
            let line: String = pts.iter().map(|(x, y)| format!(" L{x:.1} {y:.1}")).collect();
            Some(view! {
                <svg class="cloud" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
                    <path class="cloud-fill" d=format!("M{first:.1} 100{line} L{last:.1} 100 Z")/>
                    <path class="cloud-line" d=format!("M{}", &line[2..])/>
                </svg>
            })
        })
    };

    let cloud_layer = move |layer: usize| {
        move || {
            let r = shown.get()?;
            m.layers.with(|l| {
                let l = l.as_ref()?;
                let values = [&l.high, &l.mid, &l.low][layer];
                let cells = values
                    .iter()
                    .enumerate()
                    .filter_map(|(h, &v)| {
                        let (t0, t1) = (l.hour(h), l.hour(h + 1));
                        (t1 >= r.0 && t0 <= r.1 && v.is_finite()).then(|| {
                            let x = pct(t0, r) * 10.0;
                            let w = (pct(t1, r) * 10.0 - x).max(0.5);
                            view! {
                                <rect x=format!("{x:.2}") y="0" width=format!("{w:.2}") height="100" fill-opacity=format!("{:.3}", v / 100.0 * 0.9)/>
                            }
                        })
                    })
                    .collect_view();
                Some(view! { <svg class="cloud-heat" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">{cells}</svg> })
            })
        }
    };

    let axis = move || {
        let Some(r) = shown.get() else { return Vec::new().collect_view() };
        let utc = m.utc.get();
        scale::ticks(r.0, r.1, 14, utc)
            .into_iter()
            .map(|(t, midnight)| {
                view! {
                    <span class="tick" class:day=midnight style=format!("left: {:.3}%", pct(t, r))>
                        {if midnight { fmt::day(t, utc) } else { fmt::hm(t, utc) }}
                    </span>
                }
            })
            .collect_view()
    };

    // ---- Satellites ----

    let rows = move || {
        let (Some(r), Some(fc)) = (shown.get(), m.forecast.get()) else { return Vec::new().collect_view() };
        let sats = m.sats.get();
        let utc = m.utc.get();
        let unusable = m.show_unusable.get();
        let selected = m.selected_index();
        let open = m.event.get().map(|a| (a.sat, a.time));
        let labels = (r.1 - r.0) <= Duration::days(3);
        sats.iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let info = s.info;
                let acquisitions: Vec<_> = fc
                    .acquisitions
                    .iter()
                    .filter(|a| a.sat == i && a.time >= r.0 && a.time <= r.1 && (unusable || a.usable(info.kind)))
                    .cloned()
                    .collect();
                let passes: Vec<_> = fc.passes.iter().filter(|p| p.sat == i && p.los >= r.0 && p.aos <= r.1).cloned().collect();
                if acquisitions.is_empty() && passes.is_empty() {
                    return None;
                }
                let pass_bars = passes
                    .into_iter()
                    .map(|p| {
                        let (a, b) = (pct(p.aos, r).max(0.0), pct(p.los, r).min(100.0));
                        let title = format!("{} above the horizon {}–{}, up to {:.0}°", info.name, fmt::hm(p.aos, utc), fmt::hm(p.los, utc), p.max_el);
                        view! { <div class="pass" style=format!("left: {a:.3}%; width: {:.3}%", (b - a).max(0.15)) title=title></div> }
                    })
                    .collect_view();
                let marks = acquisitions
                    .into_iter()
                    .map(|a| {
                        let score = untrack(|| m.score(&a));
                        let status = untrack(|| m.plan_status(&a));
                        let usable = a.usable(info.kind);
                        let mut title = format!(
                            "{}, {} {}: score {score:.0}, {:.0}° off-nadir",
                            info.name,
                            fmt::day(a.time, utc),
                            fmt::hm(a.time, utc),
                            a.off_nadir
                        );
                        if status != Status::Unknown {
                            title.push_str(&format!(", {}", status.label()));
                        }
                        let height = 8.0 + 16.0 * score / 100.0;
                        let left = pct(a.time, r);
                        let is_open = open == Some((a.sat, a.time));
                        view! {
                            <button
                                class="cmark"
                                class:unusable=!usable
                                class:planned=matches!(status, Status::Planned(_))
                                class:unplanned=status == Status::NotPlanned
                                class:on=is_open
                                style=format!("left: {left:.3}%; height: {height:.0}px; opacity: {:.2}", 0.45 + 0.55 * score / 100.0)
                                title=title.clone()
                                aria-label=title
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:click=move |_| m.open_event(a.clone())
                            >
                                {labels.then(|| view! { <span>{format!("{score:.0}")}</span> })}
                            </button>
                        }
                    })
                    .collect_view();
                Some(view! {
                    <div class="crow" class:sel=selected == Some(i) style=format!("--sat: {}", color::css(info.color))>
                        <button class="clabel" on:click=move |_| m.select(info.norad) title=info.name>
                            {info.short}
                        </button>
                        <div class="ctrack">{pass_bars}{marks}</div>
                    </div>
                })
            })
            .collect_view()
    };

    // ---- Crosshair ----

    let now_line = move || {
        let r = shown.get()?;
        let x = pct(m.now.get(), r);
        (0.0..=100.0).contains(&x).then(|| view! { <div class="cursor" style=format!("left: {x:.3}%")></div> })
    };

    let readout = move || {
        let (t, r) = (hover.get()?, shown.get()?);
        let utc = m.utc.get();
        let x = pct(t, r);
        let sun = m.observer.with(|o| model::sun_elevation(o, t));
        let total = m.cloud_at(t);
        let layers = m.layers.with(|l| l.as_ref().and_then(|l| l.at(t)));
        let near = scaled(r.1 - r.0, 0.012);
        let events: Vec<_> = m.forecast.with(|fc| {
            fc.as_ref()
                .map(|fc| fc.acquisitions.iter().filter(|a| (a.time - t).abs() <= near).cloned().collect())
                .unwrap_or_default()
        });
        let sats = m.sats.get();
        let light = if sun < -0.833 { format!("Night, Sun {sun:.0}°") } else { format!("Sun {sun:.0}° high") }.replace('-', "−");
        let clouds = match (total, layers) {
            (Some(c), Some((lo, mi, hi))) => format!("Clouds {c:.0}%: low {lo:.0}, mid {mi:.0}, high {hi:.0}"),
            (Some(c), None) => format!("Clouds {c:.0}%"),
            _ => "No cloud forecast".into(),
        };
        let lines = events
            .iter()
            .filter_map(|a| {
                let info = sats.get(a.sat)?.info;
                let score = m.score(a);
                Some(view! {
                    <li style=format!("--sat: {}", color::css(info.color))>
                        <span class="swatch"></span>
                        {format!("{} at {}, score {score:.0}", info.name, fmt::hm(a.time, utc))}
                    </li>
                })
            })
            .collect_view();
        Some(view! {
            <div class="hair" style=format!("left: {x:.3}%")></div>
            <div class="readout-card" class:flip={x > 62.0} style=format!("left: {x:.3}%")>
                <strong>{format!("{}, {}", fmt::day(t, utc), fmt::hm(t, utc))}</strong>
                <span>{light}</span>
                <span>{clouds}</span>
                <ul>{lines}</ul>
            </div>
        })
    };

    view! {
        <section class="board panel chart">
            <div class="board-head">
                <h2>"Timeline"</h2>
                <div class="segmented" role="group" aria-label="Zoom">
                    {SPANS
                        .into_iter()
                        .map(|(label, h)| {
                            view! {
                                <button
                                    class:on=move || shown.get().is_some_and(|r| (r.1 - r.0).num_hours() == h)
                                    on:click=move |_| set_span(h)
                                >
                                    {label}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="pan">
                    <button class="tool" title="Earlier" aria-label="Earlier" on:click=move |_| pan(-0.5)>"‹"</button>
                    <button
                        class="tool"
                        title="Centre on the clock"
                        on:click=move |_| {
                            if let Some(r) = shown.get_untracked() {
                                let half = (r.1 - r.0) / 2;
                                let now = m.now.get_untracked();
                                range.set(Some((now - half, now + half)));
                            }
                        }
                    >
                        "Clock"
                    </button>
                    <button class="tool" title="Later" aria-label="Later" on:click=move |_| pan(0.5)>"›"</button>
                </div>
                <DaysPicker/>
                <p class="legend muted">
                    "Bar height is the quality score · ring: in the ESA plan · hatched: not in the plan · hollow: night"
                </p>
            </div>
            <div
                class="chart-body"
                on:pointerdown=move |ev| {
                    if let Some(r) = shown.get_untracked() {
                        drag.set_value(Some((ev.client_x() as f64, r, false)));
                        if let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                            el.set_pointer_capture(ev.pointer_id()).ok();
                        }
                    }
                }
                on:pointermove=move |ev| {
                    let x = ev.client_x() as f64;
                    if let Some((x0, r0, moved)) = drag.get_value() {
                        let dx = x - x0;
                        if moved || dx.abs() > 4.0 {
                            drag.set_value(Some((x0, r0, true)));
                            let width = track.get_untracked().map_or(1.0, |t| t.get_bounding_client_rect().width().max(1.0));
                            let shift = scaled(r0.1 - r0.0, -dx / width);
                            range.set(Some((r0.0 + shift, r0.1 + shift)));
                        }
                    }
                    hover.set(time_at(x));
                }
                on:pointerup=move |ev| {
                    if let Some((_, _, false)) = drag.get_value()
                        && let Some(t) = time_at(ev.client_x() as f64)
                    {
                        m.set_time(t);
                    }
                    drag.set_value(None);
                }
                on:pointerleave=move |_| {
                    if drag.get_value().is_none() {
                        hover.set(None);
                    }
                }
                on:wheel=move |ev| {
                    let (Some(r), Some(t)) = (shown.get_untracked(), time_at(ev.client_x() as f64)) else { return };
                    ev.prevent_default();
                    let f = 1.0015f64.powf(ev.delta_y());
                    range.set(Some((t - scaled(t - r.0, f), t + scaled(r.1 - t, f))));
                }
            >
                <div class="crow caxis">
                    <span class="clabel"></span>
                    <div class="ctrack" node_ref=track>{axis}</div>
                </div>
                <div class="crow cband">
                    <span class="clabel">"Sun"</span>
                    <div class="ctrack">{sun_band}</div>
                </div>
                <div class="crow cclouds">
                    <span class="clabel">"Clouds"</span>
                    <div class="ctrack">{cloud_total}</div>
                </div>
                <div class="crow clayer">
                    <span class="clabel muted">"High"</span>
                    <div class="ctrack">{cloud_layer(0)}</div>
                </div>
                <div class="crow clayer">
                    <span class="clabel muted">"Middle"</span>
                    <div class="ctrack">{cloud_layer(1)}</div>
                </div>
                <div class="crow clayer">
                    <span class="clabel muted">"Low"</span>
                    <div class="ctrack">{cloud_layer(2)}</div>
                </div>
                <div class="csats">
                    <div class="coverlay" aria-hidden="true">{night_bands}</div>
                    {rows}
                </div>
                <div class="coverlay ctop" aria-hidden="true">{now_line}{readout}</div>
            </div>
        </section>
    }
}
