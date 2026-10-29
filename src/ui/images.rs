//! The images already acquired over the area: when, by which satellite, how
//! cloudy, and the scenes themselves, on the page and on the globe.

use super::scale;
use crate::archive::{self, Catalogue, Mission, Pass};
use crate::fmt;
use crate::model::Model;
use chrono::Utc;
use leptos::prelude::*;

/// How far back the catalogue is searched, in days.
const SPANS: [u32; 3] = [30, 90, 180];
/// Cloud over a scene up to which an optical image counts as clear, unless
/// the slider asks for less.
const CLEAR: u32 = 20;

#[component]
pub fn Images() -> impl IntoView {
    let m = expect_context::<Model>();
    // The daily views (five a day, hundreds of metres a pixel) wait to be asked for.
    let missions = RwSignal::new(Mission::ALL.into_iter().filter(|&m| m != Mission::Daily).collect::<Vec<_>>());
    // Most cloud shown on optical scenes, in percent; 100 shows them all,
    // those with no estimate included.
    let max_cloud = RwSignal::new(100_u32);

    let shown = Memo::new(move |_| {
        let wanted = missions.get();
        let max = max_cloud.get();
        m.archive.with(|a| {
            a.passes
                .iter()
                .filter(|p| wanted.contains(&p.mission) && (max >= 100 || p.clear(f64::from(max))))
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    // What the summary counts as clear.
    let clear = Signal::derive(move || max_cloud.get().min(CLEAR));

    let toggle = move |mission: Mission| {
        missions.update(|list| match list.iter().position(|&x| x == mission) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(mission),
        })
    };

    let status = move || {
        m.archive.with(|a| {
            if a.loading {
                return Some(view! { <p class="hint">"Searching the catalogue…"</p> }.into_any());
            }
            if !a.errors.is_empty() {
                let lead = if a.passes.is_empty() { "The catalogue could not be searched" } else { "Some missions could not be searched" };
                let text = format!("{lead}: {}", a.errors.join("; "));
                return Some(view! { <p class="hint warn">{text}</p> }.into_any());
            }
            if a.passes.is_empty() {
                return a.since.map(|_| view! { <p class="hint">"No scene over this area in the catalogue for this span."</p> }.into_any());
            }
            (max_cloud.get() < 100 && shown.with(Vec::is_empty)).then(|| {
                let text = format!(
                    "No scene with at most {}% cloud in this span. Slide the cloud limit up, or search further back.",
                    max_cloud.get()
                );
                view! { <p class="hint">{text}</p> }.into_any()
            })
        })
    };

    let cards = move || shown.get().into_iter().map(|pass| view! { <Card pass=pass/> }).collect_view();

    view! {
        <section class="board panel images">
            <div class="board-head">
                <h2>"Images"</h2>
                <div class="segmented" role="group" aria-label="Span searched">
                    {SPANS
                        .into_iter()
                        .map(|d| {
                            view! {
                                <button class:on=move || m.archive_days.get() == d on:click=move |_| m.archive_days.set(d)>
                                    {format!("{d} d")}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="filters inline">
                    {Mission::ALL
                        .into_iter()
                        .map(|mission| {
                            view! {
                                <button
                                    class="chip"
                                    class:on=move || missions.with(|l| l.contains(&mission))
                                    on:click=move |_| toggle(mission)
                                >
                                    {mission.label()}
                                </button>
                            }
                        })
                        .collect_view()}
                    <label class="cloud-slider" title="Optical scenes with at most this much cloud (radar scenes stay)">
                        <span>
                            {move || match max_cloud.get() {
                                100 => "Any cloud".to_string(),
                                c => format!("Cloud ≤ {c}%"),
                            }}
                        </span>
                        <input
                            type="range"
                            min="0"
                            max="100"
                            step="5"
                            aria-label="Most cloud over a scene"
                            prop:value=move || max_cloud.get().to_string()
                            style=move || format!("--v: {}%", max_cloud.get())
                            on:input=move |e| max_cloud.set(event_target_value(&e).parse().unwrap_or(100))
                        />
                    </label>
                </div>
                <p class="legend muted">
                    {move || {
                        let from = m.archive.with(|a| a.catalogue).unwrap_or(Catalogue::PlanetaryComputer);
                        let mut text = format!(
                            "Scenes already acquired over the area, from the catalogues of {} and {} (CBERS, Amazonia), and NASA's daily views of the region (MODIS/VIIRS, last {} days). Cloud is each scene's own estimate, over the whole scene; a scene opens on the globe.",
                            from.label(),
                            Catalogue::BrazilDataCube.label(),
                            archive::DAILY_DAYS
                        );
                        if from == Catalogue::EarthSearch {
                            text.push_str(&format!(
                                " The Planetary Computer could not be reached (a privacy extension may block {}): no Sentinel-1, no Landsat previews, and only outlines on the globe.",
                                Catalogue::PlanetaryComputer.host()
                            ));
                        }
                        text
                    }}
                </p>
            </div>
            <div class="board-body">
                {status}
                <Summary clear=clear/>
                <Strip shown=shown/>
                <div class="scene-grid">{cards}</div>
            </div>
        </section>
    }
}

/// The latest clear image, and how often one comes.
#[component]
fn Summary(clear: Signal<u32>) -> impl IntoView {
    let m = expect_context::<Model>();
    move || {
        let utc = m.utc.get();
        let limit = clear.get();
        m.archive.with(|a| {
            let clear: Vec<&Pass> =
                a.passes.iter().filter(|p| p.mission.optical() && p.over_center() && p.clear(f64::from(limit))).collect();
            let days = m.archive_days.get();
            let latest = clear.first().map(|&p| {
                let pass = p.clone();
                let text = format!(
                    "Latest clear image (≤ {limit}% cloud): {}, {} ({:.0}% cloud), {}",
                    m.platform_sat(&p.platform).map_or(p.platform.as_str(), |i| i.name),
                    fmt::day(p.time, utc),
                    p.cloud().unwrap_or(0.0),
                    fmt::relative(Utc::now(), p.time)
                );
                view! {
                    <button class="banner" title="Show it on the globe" on:click=move |_| m.show_scene(pass.clone())>
                        {text}
                    </button>
                }
            });
            let rate = match clear.len() {
                _ if a.loading || a.since.is_none() => None,
                0 => Some(format!("No optical image of the area's centre with at most {limit}% cloud in {days} days.")),
                n => Some(format!(
                    "{n} optical images of the area's centre with at most {limit}% cloud in {days} days: one every {:.1} days on average.",
                    f64::from(days) / n as f64
                )),
            };
            view! {
                {latest}
                {rate.map(|r| view! { <p class="images-rate">{r}</p> })}
            }
        })
    }
}

/// The span searched, with a tick per overflight (brighter when clearer).
#[component]
fn Strip(shown: Memo<Vec<Pass>>) -> impl IntoView {
    let m = expect_context::<Model>();
    move || {
        let since = m.archive.with(|a| a.since)?;
        let now = Utc::now();
        let span = (now - since).num_seconds().max(1) as f64;
        let utc = m.utc.get();
        let ticks = shown
            .get()
            .into_iter()
            .map(|p| {
                let x = (p.time - since).num_seconds() as f64 / span * 100.0;
                let clear = p.cloud().map_or(1.0, |c| 1.0 - c / 100.0);
                let title = format!(
                    "{} {}{}",
                    p.platform,
                    fmt::day(p.time, utc),
                    p.cloud().map_or_else(String::new, |c| format!(", {c:.0}% cloud"))
                );
                let color = m.platform_color(&p.platform);
                view! {
                    <button
                        class="img-tick"
                        style=format!("left: {x:.2}%; --sat: {color}; opacity: {:.2}", 0.25 + 0.75 * clear)
                        title=title
                        on:click=move |_| m.show_scene(p.clone())
                    ></button>
                }
            })
            .collect_view();
        Some(view! {
            <div class="img-strip-wrap">
                <div class="img-strip">{ticks}</div>
                <div class="img-strip-ends muted">
                    <span>{fmt::day(since, utc)}</span>
                    <span>"today"</span>
                </div>
            </div>
        })
    }
}

#[component]
fn Card(pass: Pass) -> impl IntoView {
    let m = expect_context::<Model>();
    let scene = pass.main().clone();
    let name = m.pass_name(&pass);
    let downloads = pass.downloads();
    let color = m.platform_color(&pass.platform);
    let time = pass.time;
    let utc = m.utc;
    let cloud = pass.cloud();
    let optical = pass.mission.optical();
    let daily = pass.mission == Mission::Daily;
    let mut notes = vec![fmt::relative(Utc::now(), time)];
    if pass.scenes.len() > 1 {
        notes.push(format!("{} scenes", pass.scenes.len()));
    }
    if !pass.over_center() {
        notes.push("edge of the area".into());
    }
    let key = pass.key();
    let on = move || m.scene.with(|s| s.as_ref().is_some_and(|s| s.key() == key));
    let (show, show_button) = (pass.clone(), pass);
    let alt = format!("{name} image of {}", fmt::day(time, false));

    view! {
        <article class="scene-card" class:on=on style=format!("--sat: {color}")>
            <button class="scene-thumb" title="Show this image on the globe" on:click=move |_| m.show_scene(show.clone())>
                {scene.preview.map(|src| view! { <img src=src alt=alt loading="lazy" decoding="async"/> })}
                {match (optical, cloud) {
                    (true, Some(c)) => {
                        view! {
                            <span class="scene-cloud" style=format!("--score: {}", scale::score(100.0 - c))>
                                {format!("{c:.0}% cloud")}
                            </span>
                        }
                            .into_any()
                    }
                    (false, _) => view! { <span class="scene-cloud radar">"radar"</span> }.into_any(),
                    // The catalogue gives no cloud estimate (the Data Cube).
                    (true, None) => ().into_any(),
                }}
            </button>
            <div class="scene-meta">
                <strong>{name}</strong>
                <span>
                    {move || {
                        // A daily view's time is only the satellite's usual one.
                        let at = if daily { "daily view".to_string() } else { fmt::hm(time, utc.get()) };
                        format!("{}, {at}", fmt::day(time, utc.get()))
                    }}
                </span>
                <span class="muted">{notes.join(" · ")}</span>
            </div>
            <div class="scene-actions">
                <button class="tool" on:click=move |_| m.show_scene(show_button.clone())>"On the globe"</button>
                {scene.page.map(|href| view! { <a class="tool" href=href target="_blank" rel="noopener">"Open"</a> })}
                {(!downloads.is_empty())
                    .then(|| {
                        view! {
                            <details class="downloads">
                                <summary class="tool">"Download"</summary>
                                <ul>
                                    <li class="downloads-head">"GeoTIFF files"</li>
                                    {downloads
                                        .into_iter()
                                        .map(|d| {
                                            let file = d.clone();
                                            view! {
                                                <li>
                                                    <a
                                                        href=d.url
                                                        download
                                                        rel="noopener"
                                                        on:click=move |e| {
                                                            // The file needs its access token first.
                                                            if file.sign.is_some() {
                                                                e.prevent_default();
                                                                m.download(file.clone());
                                                            }
                                                        }
                                                    >
                                                        {d.label}
                                                    </a>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            </details>
                        }
                    })}
            </div>
        </article>
    }
}
