//! A local map of one overflight: the swath (or field of regard) over the
//! area of interest, the ground track and its direction, north and a scale.

use crate::color;
use crate::model::Model;
use chrono::Duration;
use leptos::prelude::*;
use orbitui::astro::{self, MEAN_RADIUS_KM};
use orbitui::predict::Acquisition;

const KM_PER_DEG_LAT: f64 = 110.574;
const KM_PER_DEG_LON: f64 = 111.320;

/// A line in the view's coordinates.
type Line = Vec<(f64, f64)>;

fn path(points: &[(f64, f64)], close: bool) -> String {
    let mut d: String =
        points.iter().enumerate().map(|(i, (x, y))| format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { " L" })).collect();
    if close {
        d.push_str(" Z");
    }
    d
}

#[component]
pub fn Footprint(a: Acquisition) -> impl IntoView {
    let m = expect_context::<Model>();
    let obs = m.observer.get_untracked();
    let sats = m.sats.get_untracked();
    let Some(sat) = sats.get(a.sat) else { return ().into_any() };
    let info = sat.info;
    let (lat0, lon0) = (obs.lat, obs.lon);
    let coslat = lat0.to_radians().cos();
    let to_km = |(lat, lon): (f64, f64)| (astro::wrap_lon(lon - lon0) * coslat * KM_PER_DEG_LON, (lat - lat0) * KM_PER_DEG_LAT);

    // How much ground to show: the swath, the AOI's offset and its extent.
    let area_reach = obs.area.as_ref().map_or(0.0, |area| {
        area.rings.iter().flatten().map(|&(lat, lon)| astro::central_angle(lat, lon, lat0, lon0)).fold(0.0, f64::max) * MEAN_RADIUS_KM
    });
    let reach = info.near_km.abs().max(info.far_km.abs()).max(a.cross_km.abs()).max(area_reach);
    let extent = (reach * 1.25 + 30.0).max(80.0);
    let k = 100.0 / extent;
    let to_view = |p: (f64, f64)| {
        let (x, y) = to_km(p);
        (x * k, -y * k)
    };

    // Long enough to cross the view at ~7 km/s, then some.
    let half = Duration::seconds((extent * 1.6 / 6.5) as i64 + 20);
    let track = sat.ground_track(a.time - half, a.time + half, 4);
    let mut bands: Vec<(Line, Line)> = Vec::new();
    if info.images() {
        let sides: &[f64] = if info.both_sides { &[1.0, -1.0] } else { &[1.0] };
        for &mirror in sides {
            let (mut near, mut far) = (Vec::new(), Vec::new());
            for w in track.windows(2) {
                let ((lat, lon), (lat2, lon2)) = (w[0], w[1]);
                let heading = astro::bearing(lat, lon, lat2, lon2);
                for (edge, x) in [(&mut near, info.near_km * mirror), (&mut far, info.far_km * mirror)] {
                    let side = if x >= 0.0 { 90.0 } else { -90.0 };
                    edge.push(to_view(astro::destination(lat, lon, heading + side, x.abs() / MEAN_RADIUS_KM)));
                }
            }
            bands.push((near, far));
        }
    }
    let swath = bands
        .into_iter()
        .map(|(near, mut far)| {
            far.reverse();
            path(&[near, far].concat(), true)
        })
        .collect::<Vec<_>>()
        .join(" ");
    let line: Line = track.iter().map(|&p| to_view(p)).collect();
    let arrow = line.len().checked_sub(1).map(|n| {
        let (x1, y1) = line[n];
        let (x0, y0) = line[n.saturating_sub(3)];
        let angle = (y1 - y0).atan2(x1 - x0).to_degrees();
        format!("translate({x1:.1} {y1:.1}) rotate({angle:.1})")
    });

    let aoi = match &obs.area {
        Some(area) => {
            let d = area.rings.iter().map(|r| path(&r.iter().map(|&p| to_view(p)).collect::<Vec<_>>(), true)).collect::<Vec<_>>().join(" ");
            view! { <path class="fp-aoi" d=d/> }.into_any()
        }
        None => view! { <circle class="fp-aoi-point" cx="0" cy="0" r="3.5"/> }.into_any(),
    };

    // A round length near a third of the view.
    let bar_km = [5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0].into_iter().rfind(|&l| l <= extent * 0.7).unwrap_or(5.0);
    let bar = bar_km * k;
    let label = if info.tasked() { "field of regard" } else { "swath" };

    view! {
        <figure class="footprint" style=format!("--sat: {}", color::css(info.color))>
            <svg viewBox="-104 -104 208 208" role="img" aria-label=format!("The {label} over the area of interest")>
                <defs>
                    <clipPath id="fp-clip">
                        <rect x="-100" y="-100" width="200" height="200" rx="10"/>
                    </clipPath>
                </defs>
                <rect class="fp-ground" x="-100" y="-100" width="200" height="200" rx="10"/>
                <g clip-path="url(#fp-clip)">
                    <path class="fp-swath" class:tasked=info.tasked() d=swath/>
                    <path class="fp-track" d=path(&line, false)/>
                    {arrow.map(|t| view! { <path class="fp-arrow" transform=t d="M0 0 L-9 -5 L-9 5 Z"/> })}
                    {aoi}
                </g>
                <g class="fp-north" transform="translate(84 -80)">
                    <path d="M0 -10 L5 4 L0 1 L-5 4 Z"/>
                    <text y="16">"N"</text>
                </g>
                <g class="fp-scale" transform="translate(-90 88)">
                    <line x1="0" y1="0" x2=format!("{bar:.1}") y2="0"/>
                    <text x="0" y="-5">{format!("{bar_km:.0} km")}</text>
                </g>
            </svg>
            <figcaption class="muted">{format!("The {label} over your area")}</figcaption>
        </figure>
    }
    .into_any()
}
