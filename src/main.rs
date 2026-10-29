//! EOrbit: the orbitui planner's library under a Leptos interface, on
//! a Cesium globe.

mod archive;
mod color;
mod fetch;
mod fmt;
mod globe;
mod model;
mod page;
mod quality;
mod share;
mod sky;
mod ui;

use leptos::prelude::*;
use model::{Model, Sky, View};
use orbitui::orbit::{Observer, format_coords};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let m = Model::new();
    provide_context(m);

    // A link opens what it shares, also when pasted over the page's address.
    let hash = page::hash();
    if !hash.is_empty() {
        m.apply_shared(share::decode(&hash));
    }
    let on_hash = Closure::<dyn FnMut(String)>::new(move |h: String| m.apply_shared(share::decode(&h)));
    page::on_hash_change(on_hash.as_ref().unchecked_ref());
    on_hash.forget();
    // The address follows the screen, so that it can be shared as it is.
    Effect::new(move |_| page::set_hash(&m.shared_state()));
    Effect::new(move |_| {
        if m.forecast.with(Option::is_some) {
            m.open_pending();
        }
    });
    // The acquired images are searched when looked at.
    Effect::new(move |_| {
        if m.view.get() == View::Images {
            m.observer.with(|_| ());
            m.archive_days.get();
            untrack(|| m.fetch_archive());
        }
    });

    // The globe (started by its module) reports clicks to the model.
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
        if !globe::init(on_sat.as_ref().unchecked_ref(), on_ground.as_ref().unchecked_ref()) {
            m.say("The 3D globe could not start (WebGL unavailable?); the other views still work");
        }
        on_sat.forget();
        on_ground.forget();
        m.home();
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
    Effect::new(move |_| globe::set_sky(&m.sky.get().map(Sky::tiles).unwrap_or_default()));
    Effect::new(move |_| globe::set_labels(m.labels.get()));
    Effect::new(move |_| globe::set_picking(m.picking.get()));
    Effect::new(move |_| {
        let event = m.event.get();
        m.sats.with(|s| globe::highlight(event.as_ref().and_then(|a| s.get(a.sat)), event.map_or_else(Default::default, |a| a.time)));
    });
    // The globe rests while another view covers it.
    Effect::new(move |_| {
        let on = m.view.get() == View::Globe;
        globe::set_active(on);
        if on {
            m.redraw_orbit();
        }
    });

    ui::files::accept_drops(m);
    window_event_listener(leptos::ev::keydown, move |e| {
        let typing = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"));
        if e.key() == "Escape" && !typing {
            m.reset_view();
        }
    });

    let main = move || match m.view.get() {
        View::Globe => view! {
            <ui::fleet::Fleet/>
            <ui::top::GlobeTools/>
            <ui::timeline::Timeline/>
        }
        .into_any(),
        View::Timeline => view! { <ui::chart::Chart/> }.into_any(),
        View::Calendar => view! { <ui::calendar::Calendar/> }.into_any(),
        View::Revisit => view! { <ui::revisit::Revisit/> }.into_any(),
        View::Images => view! { <ui::images::Images/> }.into_any(),
    };

    view! {
        <div class="shell" class:flat=move || m.view.get() != View::Globe>
            <ui::top::TopBar/>
            {main}
            <div class="right">
                <ui::detail::Detail/>
                <ui::agenda::Agenda/>
            </div>
        </div>
        <ui::top::Message/>
    }
}
