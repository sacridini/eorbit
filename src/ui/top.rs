//! The bar at the top: the area of interest, the map and the data status.

use crate::archive::{self, Catalogue, Mission};
use crate::coverage::{self, Ask, SOURCES};
use crate::fmt;
use crate::model::{Base, Model, Sky, View};
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::time::Duration;
use orbitui::config;
use orbitui::geocode::Place;
use orbitui::orbit::{Observer, format_coords};
use orbitui::tle::Source;

/// Typing pauses this long before the places are looked up.
const PAUSE: Duration = Duration::from_millis(250);
/// The geocoder needs this many characters.
const MIN_CHARS: usize = 2;

#[component]
pub fn TopBar() -> impl IntoView {
    let m = expect_context::<Model>();
    let query = RwSignal::new(String::new());
    let places = RwSignal::new(Vec::<Place>::new());
    // The suggestion under the arrow keys.
    let active = RwSignal::new(None::<usize>);
    let searching = RwSignal::new(false);
    // Each keystroke makes the lookups started before it stale.
    let typed = StoredValue::new(0_u64);
    let file_input = NodeRef::<html::Input>::new();

    let show = move |list: Vec<Place>| {
        active.set(None);
        places.set(list.into_iter().take(8).collect());
    };

    // Looks the query up; `enter` when the user asked for it, so that an
    // empty answer is said and a single place is taken at once.
    let lookup = move |q: String, enter: bool| {
        let stamp = typed.get_value();
        searching.set(true);
        spawn_local(async move {
            let found = Model::search(q.clone()).await;
            if typed.get_value() != stamp {
                return;
            }
            searching.set(false);
            match found {
                Ok(list) if enter && list.is_empty() => m.say(format!("No place called “{q}”")),
                Ok(list) if enter && list.len() == 1 => choose(m, &list[0], query, places),
                Ok(list) => show(list),
                Err(e) if enter => m.say(format!("Place search failed: {e}")),
                Err(_) => show(Vec::new()),
            }
        });
    };

    let on_type = move |text: String| {
        query.set(text.clone());
        typed.update_value(|n| *n += 1);
        let stamp = typed.get_value();
        let q = text.trim().to_string();
        if q.chars().count() < MIN_CHARS || config::parse_aoi(&q).is_some() {
            searching.set(false);
            show(Vec::new());
            return;
        }
        set_timeout(
            move || {
                if typed.get_value() == stamp {
                    lookup(q, false);
                }
            },
            PAUSE,
        );
    };

    let submit = move || {
        typed.update_value(|n| *n += 1);
        searching.set(false);
        if let Some(p) = places.with_untracked(|list| list.get(active.get_untracked().unwrap_or(0)).cloned()) {
            choose(m, &p, query, places);
            return;
        }
        let q = query.get_untracked().trim().to_string();
        if q.is_empty() {
            return;
        }
        if let Some((lat, lon, name)) = config::parse_aoi(&q) {
            m.set_observer(Observer::new(name, lat, lon, 0.0));
            query.set(String::new());
            return;
        }
        lookup(q, true);
    };

    let step = move |by: isize| {
        let n = places.with_untracked(Vec::len) as isize;
        if n == 0 {
            return;
        }
        let next = active.get_untracked().map_or(if by > 0 { 0 } else { n - 1 }, |i| (i as isize + by).rem_euclid(n));
        active.set(Some(next as usize));
    };

    let area_line = move || {
        m.observer.with(|o| match &o.area {
            Some(a) => format!("{:.0} km² around {}", a.area_km2, o.coords()),
            None => o.coords(),
        })
    };

    view! {
        <header class="top panel">
            <div class="wordmark">"EOrbit"</div>
            <nav class="tabs" aria-label="Views">
                {View::ALL
                    .into_iter()
                    .map(|v| {
                        view! {
                            <button
                                class:on=move || m.view.get() == v
                                aria-current=move || (m.view.get() == v).then_some("page")
                                on:click=move |_| m.view.set(v)
                            >
                                {v.label()}
                            </button>
                        }
                    })
                    .collect_view()}
            </nav>
            <div class="aoi">
                <div class="aoi-now">
                    <span class="aoi-name">{move || m.observer.with(|o| o.name.clone())}</span>
                    <span class="aoi-coords">{area_line}</span>
                </div>
                <div class="search" class:busy=searching>
                    <input
                        type="search"
                        placeholder="Search a place, or type lat, lon"
                        aria-label="Area of interest"
                        prop:value=query
                        autocomplete="off"
                        aria-autocomplete="list"
                        aria-expanded=move || places.with(|p| !p.is_empty()).to_string()
                        on:input=move |ev| on_type(event_target_value(&ev))
                        on:keydown=move |ev| match ev.key().as_str() {
                            "Enter" => submit(),
                            "ArrowDown" => {
                                ev.prevent_default();
                                step(1);
                            }
                            "ArrowUp" => {
                                ev.prevent_default();
                                step(-1);
                            }
                            "Escape" => show(Vec::new()),
                            _ => {}
                        }
                        // Late enough for a click on a suggestion to land.
                        on:blur=move |_| set_timeout(move || show(Vec::new()), Duration::from_millis(200))
                        on:focus=move |_| {
                            let q = query.get_untracked();
                            if places.with_untracked(Vec::is_empty) && q.trim().chars().count() >= MIN_CHARS {
                                on_type(q);
                            }
                        }
                    />
                    <Show when=move || places.with(|p| !p.is_empty())>
                        <ul class="places" role="listbox">
                            {move || {
                                places
                                    .get()
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, p)| {
                                        let label = p.label();
                                        let coords = format_coords(p.lat, p.lon);
                                        view! {
                                            <li role="option" aria-selected=move || (active.get() == Some(i)).to_string()>
                                                <button
                                                    tabindex="-1"
                                                    class:on=move || active.get() == Some(i)
                                                    on:mouseenter=move |_| active.set(Some(i))
                                                    on:click=move |_| {
                                                        typed.update_value(|n| *n += 1);
                                                        searching.set(false);
                                                        choose(m, &p, query, places);
                                                    }
                                                >
                                                    <span>{label}</span>
                                                    <span class="muted">{coords}</span>
                                                </button>
                                            </li>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </ul>
                    </Show>
                </div>
                <button
                    class="tool pick"
                    class:on=m.picking
                    title="Click the globe to place the area"
                    on:click=move |_| m.picking.update(|p| *p = !*p)
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                        <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" />
                        <circle cx="12" cy="10" r="2.3" />
                    </svg>
                    <span>"Pick"</span>
                </button>
                <button
                    class="tool"
                    title="Load a GeoJSON polygon (or drop the file on the page)"
                    on:click=move |_| {
                        if let Some(input) = file_input.get() {
                            input.click();
                        }
                    }
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                        <path d="M5 18 9 6l5 9 2-4 3 7z" />
                    </svg>
                    <span>"GeoJSON"</span>
                </button>
                <input
                    type="file"
                    accept=".geojson,.json,application/geo+json"
                    hidden
                    node_ref=file_input
                    on:change=move |_| {
                        let file = file_input.get().and_then(|i| i.files()).and_then(|f| f.get(0));
                        if let Some(file) = file {
                            super::files::open(m, file);
                        }
                    }
                />
            </div>
            <div class="top-right">
                <div class="segmented" role="group" aria-label="Base map">
                    <button class:on=move || m.base.get() == Base::Imagery on:click=move |_| m.base.set(Base::Imagery)>
                        "Imagery"
                    </button>
                    <button class:on=move || m.base.get() == Base::Map on:click=move |_| m.base.set(Base::Map)>
                        "Map"
                    </button>
                </div>
                <button class="tool" title="Copy a link to this area, view and opportunity" on:click=move |_| m.share()>
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                        <path d="M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4l-1 1"/>
                        <path d="M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1-1"/>
                    </svg>
                    <span>"Share"</span>
                </button>
                <button class="tool" class:on=m.labels title="Satellite names on the globe" on:click=move |_| m.labels.update(|l| *l = !*l)>
                    "Names"
                </button>
                <DataStatus/>
            </div>
        </header>
    }
}

fn choose(m: Model, p: &Place, query: RwSignal<String>, places: RwSignal<Vec<Place>>) {
    m.set_observer(Observer::new(p.label(), p.lat, p.lon, p.elevation));
    places.set(Vec::new());
    query.set(String::new());
}

/// Buttons floating over the globe.
#[component]
pub fn GlobeTools() -> impl IntoView {
    let m = expect_context::<Model>();
    view! {
        <div class="stage">
            <button
                class="tool floating"
                title="Back to the whole area, with no satellite selected (Esc)"
                on:click=move |_| m.reset_view()
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M4 12a8 8 0 1 0 2.4-5.7"/>
                    <path d="M4 4v4.5h4.5"/>
                </svg>
                <span>"Reset view"</span>
            </button>
            <SkyTool/>
            <CoverageTool/>
            <SceneChip/>
        </div>
    }
}

/// NASA's view of a recent day over the whole globe: which day, which
/// satellite.
#[component]
fn SkyTool() -> impl IntoView {
    let m = expect_context::<Model>();
    let step = move |by: i64| m.sky.update(|s| if let Some(s) = s { s.back = (s.back + by).clamp(0, Sky::MAX_BACK) });
    move || match m.sky.get() {
        None => view! {
            <button
                class="tool floating"
                title="Lay NASA's view of a recent day over the whole globe (MODIS, VIIRS), to see its clouds"
                on:click=move |_| m.sky.set(Some(Sky::LATEST))
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M7 18h10a4 4 0 0 0 .5-8 6 6 0 0 0-11.5 1.5A3.3 3.3 0 0 0 7 18z"/>
                </svg>
                <span>"Daily view"</span>
            </button>
        }
        .into_any(),
        Some(sky) => {
            let day = if sky.back == 0 { "today, so far".to_string() } else { sky.date().format("%a %-d %b").to_string() };
            view! {
                <div class="sky-tool floating" role="group" aria-label="NASA's daily view">
                    <button class="step" title="The day before" aria-label="The day before" disabled={sky.back >= Sky::MAX_BACK} on:click=move |_| step(1)>
                        "‹"
                    </button>
                    <span class="sky-day">{day}</span>
                    <button class="step" title="The day after" aria-label="The day after" disabled={sky.back == 0} on:click=move |_| step(-1)>
                        "›"
                    </button>
                    <select
                        aria-label="Satellite"
                        on:change=move |e| {
                            let layer = event_target_value(&e).parse().unwrap_or(Sky::LATEST.layer);
                            m.sky.update(|s| if let Some(s) = s { s.layer = layer });
                        }
                    >
                        {archive::DAILY
                            .iter()
                            .enumerate()
                            .map(|(i, (sat, camera, ..))| {
                                view! { <option value=i.to_string() selected={i == sky.layer}>{format!("{sat} {camera}")}</option> }
                            })
                            .collect_view()}
                    </select>
                    <button class="close" title="Take the daily view off" aria-label="Take the daily view off" on:click=move |_| m.sky.set(None)>
                        "×"
                    </button>
                </div>
            }
            .into_any()
        }
    }
}

/// The coverage map: how many scenes a satellite acquired in each cell of
/// the world in the last months.
#[component]
fn CoverageTool() -> impl IntoView {
    let m = expect_context::<Model>();
    let set = move |f: &dyn Fn(&mut Ask)| m.coverage_ask.update(|a| if let Some(a) = a { f(a) });
    move || match m.coverage_ask.get() {
        None => view! {
            <button
                class="tool floating"
                title="How many scenes each satellite acquired over the world in the last months, by cell"
                on:click=move |_| m.coverage_ask.set(Some(Ask::DEFAULT))
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M7 3.5l4 2.3v4.6l-4 2.3-4-2.3V5.8zM17 3.5l4 2.3v4.6l-4 2.3-4-2.3V5.8zM12 12l4 2.3v4.6L12 21.2l-4-2.3v-4.6z"/>
                </svg>
                <span>"Coverage"</span>
            </button>
        }
        .into_any(),
        Some(ask) => {
            let groups = coverage::MISSIONS.map(|mission| {
                let options = SOURCES
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.mission == mission)
                    .map(|(i, s)| {
                        let label = if s.label == "All" { format!("{mission}, all satellites") } else { s.label.to_string() };
                        view! { <option value=i.to_string() selected={i == ask.source}>{label}</option> }
                    })
                    .collect_view();
                view! { <optgroup label=mission>{options}</optgroup> }
            });
            view! {
                <div class="coverage floating">
                    <div class="sky-tool" role="group" aria-label="Coverage map">
                        <select
                            aria-label="Satellite"
                            on:change=move |e| {
                                let i = event_target_value(&e).parse().unwrap_or(0);
                                set(&|a| a.source = i);
                            }
                        >
                            {groups.into_iter().collect_view()}
                        </select>
                        <div class="segmented" role="group" aria-label="Months counted">
                            {coverage::MONTHS
                                .into_iter()
                                .map(|n| {
                                    view! {
                                        <button class:on={ask.months == n} title=format!("The last {}", Ask { months: n, ..ask }.span_label()) on:click=move |_| set(&|a| a.months = n)>
                                            {format!("{n} mo")}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>
                        <button class="close" title="Take the coverage map off" aria-label="Take the coverage map off" on:click=move |_| m.coverage_ask.set(None)>
                            "×"
                        </button>
                    </div>
                    <CoverageLegend/>
                </div>
            }
            .into_any()
        }
    }
}

/// What the colours of the coverage map count.
#[component]
fn CoverageLegend() -> impl IntoView {
    let m = expect_context::<Model>();
    move || {
        m.coverage.with(|c| {
            let ask = c.ask?;
            if c.loading {
                let text = if ask.aggregated() { "Counting the scenes…" } else { "Listing INPE's scenes (up to half a minute)…" };
                return Some(view! { <p class="coverage-note">{text}</p> }.into_any());
            }
            if let Some(e) = &c.error {
                return Some(view! { <p class="coverage-note warn">{e.clone()}</p> }.into_any());
            }
            if c.counts.cells.is_empty() {
                return Some(view! { <p class="coverage-note">"No scene in this span."</p> }.into_any());
            }
            let breaks = c.counts.breaks();
            let classes = coverage::RAMP
                .iter()
                .take(breaks.len() + 1)
                .enumerate()
                .map(|(i, color)| {
                    let from = if i == 0 { 1 } else { breaks[i - 1] };
                    let range = match breaks.get(i) {
                        Some(&to) if to - 1 > from => format!("{from}–{}", to - 1),
                        Some(_) => from.to_string(),
                        None => format!("{from}+"),
                    };
                    view! {
                        <li>
                            <span class="swatch" style=format!("background: {color}")></span>
                            <span>{range}</span>
                        </li>
                    }
                })
                .collect_view();
            let how = if ask.aggregated() {
                "each counted in the cell that holds its centre"
            } else {
                "each counted in every cell whose centre it covers"
            };
            let total = format!(
                "{} scenes of {} in the last {}, {how} (cells of about {} km²).",
                fmt::thousands(c.counts.scenes),
                ask.source().name(),
                ask.span_label(),
                fmt::thousands(coverage::CELL_KM2)
            );
            let from = if ask.aggregated() { Catalogue::EarthSearch } else { Catalogue::BrazilDataCube }.label();
            let note = format!("{from}. Hover a cell for its count; zoom in to read them all.");
            Some(
                view! {
                    <div class="coverage-legend">
                        <ol class="coverage-ramp" aria-label="Scenes per cell">{classes}</ol>
                        <p class="coverage-note">{total}</p>
                        <p class="coverage-note muted">{note}</p>
                    </div>
                }
                .into_any(),
            )
        })
    }
}

/// The acquired image draped over the globe, and how to take it off.
#[component]
fn SceneChip() -> impl IntoView {
    let m = expect_context::<Model>();
    move || {
        let pass = m.scene.get()?;
        let utc = m.utc.get();
        let name = m.pass_name(&pass);
        let mut text = if pass.mission == Mission::Daily {
            format!("{name} daily view of {}", fmt::day(pass.time, utc))
        } else {
            format!("{name} image of {}, {}", fmt::day(pass.time, utc), fmt::hm(pass.time, utc))
        };
        if let Some(c) = pass.cloud().filter(|_| pass.mission.optical()) {
            text.push_str(&format!(", {c:.0}% cloud"));
        }
        let color = m.platform_color(&pass.platform);
        Some(view! {
            <div class="scene-chip floating" style=format!("--sat: {color}")>
                <span class="swatch"></span>
                <span>{text}</span>
                <button class="close" title="Take the image off the globe" aria-label="Take the image off the globe" on:click=move |_| m.hide_scene()>
                    "×"
                </button>
            </div>
        })
    }
}

/// Where the elements come from, and the forecast's progress.
#[component]
fn DataStatus() -> impl IntoView {
    let m = expect_context::<Model>();
    let elements = move || {
        let s = m.elements_status.get();
        if s.updating {
            return "Updating orbits…".to_string();
        }
        match (s.source, s.fetched) {
            (Source::Snapshot, _) | (_, None) => "Orbits from the bundled snapshot".into(),
            (_, Some(t)) => format!("Orbits from Celestrak, {}", fmt::age(t)),
        }
    };
    view! {
        <div class="status">
            <button class="status-line" title="Download the latest orbital elements" on:click=move |_| m.refresh_elements()>
                <span class="dot" class:busy=move || m.elements_status.get().updating></span>
                {elements}
            </button>
            <Show when=move || m.progress.get().is_some()>
                <div class="progress" title="Computing the forecast">
                    <div class="bar" style:width=move || format!("{:.0}%", m.progress.get().unwrap_or(0.0) * 100.0)></div>
                </div>
            </Show>
        </div>
    }
}

/// Short-lived messages.
#[component]
pub fn Message() -> impl IntoView {
    let m = expect_context::<Model>();
    view! {
        <Show when=move || m.message.with(Option::is_some)>
            <div class="toast" role="status">{move || m.message.get().map(|(t, _)| t)}</div>
        </Show>
    }
}
