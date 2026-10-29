//! The satellites tracked, and the mission groups shown.

use crate::color;
use crate::fmt;
use crate::model::Model;
use leptos::prelude::*;
use orbitui::catalog::{self, Kind};

#[component]
pub fn Fleet() -> impl IntoView {
    let m = expect_context::<Model>();
    let groups_open = RwSignal::new(false);

    let rows = move || {
        let sats = m.sats.get();
        let selected = m.selected.get();
        sats.iter()
            .enumerate()
            .map(|(i, s)| {
                let info = s.info;
                let norad = info.norad;
                let next = move || {
                    let now = m.now.get();
                    m.forecast.with(|fc| {
                        fc.as_ref().and_then(|fc| fc.next_acquisition(i, now, info.kind)).map(|a| fmt::relative(now, a.time))
                    })
                };
                let kind = match info.kind {
                    Kind::Optical => "optical",
                    Kind::Sar => "radar",
                    Kind::Lidar => "lidar",
                    Kind::Station => "station",
                };
                let detail = if info.images() { format!("{} · {kind}", info.sensor) } else { "tracking only".into() };
                view! {
                    <li>
                        <button
                            class="sat"
                            class:on=selected == Some(norad)
                            style=format!("--sat: {}", color::css(info.color))
                            on:click=move |_| m.select(norad)
                            title=info.access()
                        >
                            <span class="swatch"></span>
                            <span class="sat-name">
                                <strong>{info.name}</strong>
                                <span class="muted">{detail}</span>
                            </span>
                            <span class="sat-next">{next}</span>
                        </button>
                    </li>
                }
            })
            .collect_view()
    };

    view! {
        <aside class="fleet panel">
            <div class="panel-head">
                <h2>"Satellites"</h2>
                <span class="count">{move || m.sats.with(|s| s.len())}</span>
                <button class="ghost" class:on=groups_open on:click=move |_| groups_open.update(|o| *o = !*o)>
                    "Missions"
                </button>
            </div>
            <Show when=move || groups_open.get()>
                <Groups/>
            </Show>
            <p class="hint">"Next usable imaging of your area"</p>
            <ul class="sat-list">{rows}</ul>
        </aside>
    }
}

/// Switches the built-in mission groups on and off.
#[component]
fn Groups() -> impl IntoView {
    let m = expect_context::<Model>();
    let enabled = RwSignal::new(catalog::read_user().map(|f| f.enabled_groups()).unwrap_or_else(|_| catalog::default_groups()));
    let toggle = move |id: &'static str| {
        enabled.update(|list| {
            if let Some(i) = list.iter().position(|g| g == id) {
                list.remove(i);
            } else {
                list.push(id.to_string());
            }
        });
        match catalog::set_groups(enabled.get_untracked()) {
            Ok(()) => m.reload_catalog(),
            Err(e) => m.say(e),
        }
    };
    view! {
        <ul class="groups">
            {catalog::GROUPS
                .iter()
                .map(|g| {
                    let id = g.id;
                    view! {
                        <li>
                            <label>
                                <input
                                    type="checkbox"
                                    prop:checked=move || enabled.with(|l| l.iter().any(|e| e == id))
                                    on:change=move |_| toggle(id)
                                />
                                <span>{g.name}</span>
                            </label>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}
