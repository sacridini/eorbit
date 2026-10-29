//! Upcoming imaging opportunities over the area of interest.

use crate::color;
use crate::fmt;
use crate::model::Model;
use leptos::prelude::*;

const ROWS: usize = 60;

#[component]
pub fn Agenda() -> impl IntoView {
    let m = expect_context::<Model>();

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
                                </span>
                            </span>
                            <span class="event-in">{move || fmt::relative(m.now.get(), time)}</span>
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
            </div>
            <div class="filters">
                <button class="chip" class:on=m.only_selected on:click=move |_| m.only_selected.update(|v| *v = !*v)>
                    "Selected satellite"
                </button>
                <button class="chip" class:on=m.show_unusable on:click=move |_| m.show_unusable.update(|v| *v = !*v)>
                    "Include night passes"
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
