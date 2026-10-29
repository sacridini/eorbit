//! orbitglobe: the orbitui planner's library under a Leptos interface, on
//! a Cesium globe.

mod color;
mod fetch;
mod fmt;
mod globe;
mod model;
mod ui;

use leptos::prelude::*;
use model::Model;
use orbitui::orbit::{Observer, format_coords};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let m = Model::new();
    provide_context(m);

    // The globe, once its container is in the page.
    Effect::new(move |_| {
        let on_sat = Closure::<dyn FnMut(u32)>::new(move |i: u32| {
            if let Some(norad) = m.sats.with_untracked(|s| s.get(i as usize).map(|s| s.info.norad)) {
                m.select(norad);
            }
        });
        let on_ground = Closure::<dyn FnMut(f64, f64)>::new(move |lat: f64, lon: f64| {
            if m.picking.get_untracked() {
                m.picking.set(false);
                m.set_observer(Observer::new(format_coords(lat, lon), lat, lon, 0.0));
            }
        });
        globe::init("globe", "credits", on_sat.as_ref().unchecked_ref(), on_ground.as_ref().unchecked_ref());
        on_sat.forget();
        on_ground.forget();
        let obs = m.observer.get_untracked();
        globe::fly_to(obs.lat, obs.lon, 16_000_000.0, 0.0);
        m.run();
    });

    // The globe follows the state.
    Effect::new(move |_| {
        let sats = m.sats.get();
        globe::fleet(&sats);
        m.redraw_orbit();
    });
    Effect::new(move |_| globe::set_selected(m.selected_index().map_or(-1, |i| i as i32)));
    Effect::new(move |_| m.observer.with(globe::aoi));
    Effect::new(move |_| globe::set_base(m.base.get().id()));
    Effect::new(move |_| globe::set_labels(m.labels.get()));
    Effect::new(move |_| globe::set_picking(m.picking.get()));
    Effect::new(move |_| {
        let event = m.event.get();
        m.sats.with(|s| globe::highlight(event.as_ref().and_then(|a| s.get(a.sat)), event.map_or_else(Default::default, |a| a.time)));
    });

    ui::files::accept_drops(m);

    view! {
        <div id="globe"></div>
        <div class="shell">
            <ui::top::TopBar/>
            <ui::fleet::Fleet/>
            <div class="right">
                <ui::detail::Detail/>
                <ui::agenda::Agenda/>
            </div>
            <ui::timeline::Timeline/>
        </div>
        <ui::top::Message/>
        <div id="credits"></div>
    }
}
