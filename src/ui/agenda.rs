//! Upcoming imaging opportunities over the area of interest, their export,
//! and the ESA acquisition plans that confirm them.

use super::scale;
use crate::color;
use crate::fmt;
use crate::model::{Model, Order};
use leptos::html;
use leptos::prelude::*;
use orbitui::plan::{self, Status};

const ROWS: usize = 60;

#[component]
pub fn Agenda() -> impl IntoView {
    let m = expect_context::<Model>();
    let plans_open = RwSignal::new(false);
    let export_open = RwSignal::new(false);

    // The list is rebuilt when its content changes, not at every tick.
    let keys = Memo::new(move |_| m.upcoming(ROWS).iter().map(|a| (a.sat, a.time.timestamp_millis())).collect::<Vec<_>>());
    let rows = move || {
        let _ = keys.get();
        let utc = m.utc.get();
        let list = untrack(|| m.upcoming(ROWS));
        let sats = m.sats.get();
        let open = m.event.get().map(|a| (a.sat, a.time));
        list.into_iter()
            .filter_map(|a| {
                let info = sats.get(a.sat)?.info;
                let usable = a.usable(info.kind);
                let cloud = m.cloud_at(a.time).map(|c| format!("{c:.0}% cloud"));
                let score = m.score(&a);
                let plan = m.plan_status(&a);
                let side = if a.cross_km >= 0.0 { "right" } else { "left" };
                let title = format!(
                    "{:.0} km {side} of the track, {} pass, satellite {:.0}° above the horizon",
                    a.cross_km.abs(),
                    if a.ascending { "ascending" } else { "descending" },
                    a.elevation
                );
                let time = a.time;
                Some(view! {
                    <li>
                        <button
                            class="event"
                            class:on=open == Some((a.sat, a.time))
                            class:unusable=!usable
                            style=format!("--sat: {}", color::css(info.color))
                            title=title
                            on:click=move |_| m.open_event(a.clone())
                        >
                            <span class="event-when">
                                <strong>{fmt::hm(time, utc)}</strong>
                                <span class="muted">{fmt::day(time, utc)}</span>
                            </span>
                            <span class="event-what">
                                <strong>{info.name}</strong>
                                <span class="facts">
                                    <span>{format!("{:.0}° off-nadir", a.off_nadir)}</span>
                                    {(a.coverage < 0.995).then(|| view! { <span>{format!("{:.0}% of the area", a.coverage * 100.0)}</span> })}
                                    {cloud.map(|c| view! { <span>{c}</span> })}
                                    {(!usable).then(|| view! { <span class="warn">"night"</span> })}
                                    {match plan {
                                        Status::Planned(_) => Some(view! { <span class="plan yes">{plan.label()}</span> }),
                                        Status::NotPlanned => Some(view! { <span class="plan no">{plan.label()}</span> }),
                                        Status::Unknown => None,
                                    }}
                                </span>
                            </span>
                            <span class="event-side">
                                <span class="score-pill" style=format!("--score: {}", scale::score(score)) title="Quality score">
                                    {format!("{score:.0}")}
                                </span>
                                <span class="event-in">{move || fmt::relative(m.now.get(), time)}</span>
                            </span>
                        </button>
                    </li>
                })
            })
            .collect_view()
    };

    let empty = move || m.forecast.with(|f| f.is_some()) && m.upcoming(1).is_empty();

    view! {
        <section class="agenda panel">
            <div class="panel-head">
                <h2>"Imaging opportunities"</h2>
                <button
                    class="ghost"
                    class:on=plans_open
                    title="ESA acquisition plans"
                    on:click=move |_| plans_open.update(|o| *o = !*o)
                >
                    "Plans"
                    {move || {
                        let n = m.plans.with(|p| p.files.len());
                        (n > 0).then(|| view! { <span class="badge">{n}</span> })
                    }}
                </button>
                <button class="ghost" class:on=export_open on:click=move |_| export_open.update(|o| *o = !*o)>
                    "Export"
                </button>
            </div>
            <Show when=move || export_open.get()>
                <div class="popover">
                    <p class="muted">"The opportunities listed (with these filters), with their geometry, clouds and plan."</p>
                    <div class="row-buttons">
                        <button class="tool" on:click=move |_| m.export(false)>"Spreadsheet (CSV)"</button>
                        <button class="tool" on:click=move |_| m.export(true)>"Calendar (ICS)"</button>
                    </div>
                </div>
            </Show>
            <Show when=move || plans_open.get()>
                <Plans/>
            </Show>
            <div class="filters">
                <div class="segmented" role="group" aria-label="Order">
                    <button class:on=move || m.order.get() == Order::Soonest on:click=move |_| m.order.set(Order::Soonest)>
                        "Soonest"
                    </button>
                    <button class:on=move || m.order.get() == Order::Best on:click=move |_| m.order.set(Order::Best)>
                        "Best"
                    </button>
                </div>
                <button class="chip" class:on=m.only_selected on:click=move |_| m.only_selected.update(|v| *v = !*v)>
                    "Selected satellite"
                </button>
                <button class="chip" class:on=m.show_unusable on:click=move |_| m.show_unusable.update(|v| *v = !*v)>
                    "Night passes"
                </button>
            </div>
            <Show when=move || m.forecast.with(Option::is_none)>
                <p class="hint">
                    {move || format!("Forecasting the next {} days…", m.days.get())}
                </p>
            </Show>
            <Show when=empty>
                <p class="hint">"No opportunity in the forecast with these filters."</p>
            </Show>
            <ol class="events">{rows}</ol>
        </section>
    }
}

/// The ESA plans loaded, and how to get them.
#[component]
fn Plans() -> impl IntoView {
    let m = expect_context::<Model>();
    let input = NodeRef::<html::Input>::new();
    let files = move || {
        m.plans.with(|p| {
            let utc = m.utc.get();
            p.files
                .iter()
                .map(|f| {
                    let name = f.name.clone();
                    view! {
                        <li>
                            <span>
                                <strong>{f.sat.clone()}</strong>
                                {format!(" {} to {}, {} segments", fmt::day(f.start, utc), fmt::day(f.end, utc), f.segments.len())}
                            </span>
                            <button class="close" title="Remove this plan" aria-label="Remove this plan" on:click=move |_| m.remove_plan(&name)>
                                "×"
                            </button>
                        </li>
                    }
                })
                .collect_view()
        })
    };
    view! {
        <div class="popover">
            <p class="muted">
                "Sentinel-1 and Sentinel-2 publish what they will acquire. Download their KML plans from ESA and drop them on the page: opportunities then say whether they are planned."
            </p>
            <div class="row-buttons">
                <a class="tool" href=plan::PAGES[1] target="_blank" rel="noopener">"Sentinel-1 plans"</a>
                <a class="tool" href=plan::PAGES[0] target="_blank" rel="noopener">"Sentinel-2 plans"</a>
                <button
                    class="tool"
                    on:click=move |_| {
                        if let Some(i) = input.get() {
                            i.click();
                        }
                    }
                >
                    "Load KML…"
                </button>
                <input
                    type="file"
                    accept=".kml"
                    multiple
                    hidden
                    node_ref=input
                    on:change=move |_| super::files::open_all(m, input.get().and_then(|i| i.files()))
                />
            </div>
            <ul class="plan-files">{files}</ul>
        </div>
    }
}
