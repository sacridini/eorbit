//! The bar at the top: the area of interest, the map and the data status.

use crate::fmt;
use crate::model::{Base, Model, View};
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
                {[(View::Globe, "Globe"), (View::Timeline, "Timeline"), (View::Calendar, "Calendar"), (View::Revisit, "Revisit")]
                    .into_iter()
                    .map(|(v, label)| {
                        view! {
                            <button
                                class:on=move || m.view.get() == v
                                aria-current=move || (m.view.get() == v).then_some("page")
                                on:click=move |_| m.view.set(v)
                            >
                                {label}
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
        </div>
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
