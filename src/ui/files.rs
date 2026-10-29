//! GeoJSON areas of interest dropped on the page or chosen from disk.

use crate::fetch;
use crate::model::Model;
use leptos::prelude::*;
use leptos::task::spawn_local;
use orbitui::config;

pub fn open(m: Model, file: web_sys::File) {
    let stem = config::file_stem(&file.name());
    spawn_local(async move {
        match fetch::file(file).await {
            Ok(text) => m.load_geojson(&text, &stem),
            Err(e) => m.say(e),
        }
    });
}

/// GeoJSON files dropped anywhere become the area of interest.
pub fn accept_drops(m: Model) {
    window_event_listener(leptos::ev::dragover, |e| e.prevent_default());
    window_event_listener(leptos::ev::drop, move |e| {
        e.prevent_default();
        if let Some(file) = e.data_transfer().and_then(|d| d.files()).and_then(|f| f.get(0)) {
            open(m, file);
        }
    });
}
