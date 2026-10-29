//! One opportunity in detail: its geometry, light and weather, and the
//! satellite's path across the sky of the area.

use crate::color;
use crate::fmt;
use super::footprint::Footprint;
use super::scale;
use crate::model::{self, Model};
use crate::quality;
use chrono::Duration;
use leptos::prelude::*;
use orbitui::plan::Status;
use orbitui::predict::{self, Acquisition};

#[component]
pub fn Detail() -> impl IntoView {
    let m = expect_context::<Model>();
    move || m.event.get().map(|a| view! { <Card a=a/> })
}

#[component]
fn Card(a: Acquisition) -> impl IntoView {
    let m = expect_context::<Model>();
    let Some(sat) = m.sats.with_untracked(|s| s.get(a.sat).map(|s| (s.info, s.period_minutes()))) else {
        return ().into_any();
    };
    let (info, _) = sat;
    let utc = m.utc;
    let obs = m.observer.get_untracked();
    let cloud = untrack(|| m.cloud_at(a.time));
    let light = model::sun_elevation(&obs, a.time);
    let side = if a.cross_km >= 0.0 { "right" } else { "left" };
    let time = a.time;
    let factors = quality::factors(&a, info.kind, cloud);
    let score = factors.score();
    let plan = untrack(|| m.plan_status(&a));
    let why = format!(
        "Clear sky {}, sunlight {:.0}%, viewing angle {:.0}%, area imaged {:.0}%",
        factors.clear.map_or_else(|| "unknown".to_string(), |c| format!("{:.0}%", c * 100.0)),
        factors.sun * 100.0,
        factors.look * 100.0,
        factors.cover * 100.0
    );

    let facts = [
        ("Off-nadir", format!("{:.1}°", a.off_nadir)),
        ("From the track", format!("{:.0} km {side}", a.cross_km.abs())),
        ("Area imaged", format!("{:.0}%", a.coverage * 100.0)),
        ("Cloud cover", cloud.map_or_else(|| "no forecast".into(), |c| format!("{c:.0}%"))),
        ("Sun height", format!("{light:.0}°").replace('-', "−")),
        ("Pass", if a.ascending { "ascending".to_string() } else { "descending".to_string() }),
        (
            "ESA plan",
            match &plan {
                Status::Unknown => "no plan loaded".to_string(),
                other => other.label(),
            },
        ),
    ];

    view! {
        <section class="detail panel" style=format!("--sat: {}", color::css(info.color))>
            <div class="detail-head">
                <div>
                    <h2>{info.name}</h2>
                    <p class="muted">{format!("{} · {}", info.sensor, info.access())}</p>
                </div>
                <button class="close" title="Close" aria-label="Close" on:click=move |_| m.event.set(None)>
                    "×"
                </button>
            </div>
            <div class="detail-time">
                <span class="big">{move || fmt::hm(time, utc.get())}</span>
                <span>
                    {move || format!("{}, {}", fmt::day(time, utc.get()), fmt::zone(time, utc.get()))}
                    <br/>
                    <span class="muted">{move || fmt::relative(m.now.get(), time)}</span>
                </span>
                <span class="score-badge" style=format!("--score: {}", scale::score(score)) title=why>
                    <strong>{format!("{score:.0}")}</strong>
                    <span>"score"</span>
                </span>
            </div>
            <dl class="facts-grid">
                {facts
                    .into_iter()
                    .map(|(k, v)| view! { <div><dt>{k}</dt><dd>{v}</dd></div> })
                    .collect_view()}
            </dl>
            <div class="detail-figures">
                <Footprint a=a.clone()/>
                <SkyPlot a=a/>
            </div>
        </section>
    }
    .into_any()
}

/// Azimuth/elevation to plot coordinates (zenith at the centre, north up).
fn polar(az: f64, el: f64) -> (f64, f64) {
    let r = (90.0 - el.max(0.0)) / 90.0 * 100.0;
    let az = az.to_radians();
    (r * az.sin(), -r * az.cos())
}

#[component]
fn SkyPlot(a: Acquisition) -> impl IntoView {
    let m = expect_context::<Model>();
    let obs = m.observer.get_untracked();
    let sats = m.sats.get_untracked();
    let Some(sat) = sats.get(a.sat) else { return ().into_any() };
    let track = predict::sky_track(sat, &obs, a.time - Duration::minutes(12), a.time + Duration::minutes(12), 10);
    let path: String = track
        .iter()
        .enumerate()
        .map(|(i, &(az, el))| {
            let (x, y) = polar(az, el);
            format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { " L" })
        })
        .collect();
    let (ax, ay) = polar(a.azimuth, a.elevation);

    // Where the satellite is now, when it is above the horizon.
    let here = move || {
        let now = m.now.get();
        let st = sats.get(a.sat)?.state(now)?;
        let look = obs.look(st.ecef);
        (look.el >= 0.0).then(|| polar(look.az, look.el))
    };

    view! {
        <figure class="sky">
            <svg viewBox="-118 -118 236 236" role="img" aria-label="Path of the satellite across the sky of the area">
                <circle class="ring" r="100"/>
                <circle class="ring" r="66.7"/>
                <circle class="ring" r="33.3"/>
                <line class="ring" x1="-100" y1="0" x2="100" y2="0"/>
                <line class="ring" x1="0" y1="-100" x2="0" y2="100"/>
                <text class="cardinal" x="0" y="-105">"N"</text>
                <text class="cardinal" x="109" y="4">"E"</text>
                <text class="cardinal" x="0" y="114">"S"</text>
                <text class="cardinal" x="-109" y="4">"W"</text>
                <path class="sky-path" d=path/>
                <circle class="sky-peak" cx=format!("{ax:.1}") cy=format!("{ay:.1}") r="4"/>
                {move || here().map(|(x, y)| view! { <circle class="sky-now" cx=format!("{x:.1}") cy=format!("{y:.1}") r="5.5"/> })}
            </svg>
            <figcaption class="muted">{format!("{:.0}° high at the acquisition", a.elevation)}</figcaption>
        </figure>
    }
    .into_any()
}
