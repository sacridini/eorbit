//! Files chosen or dropped on the page: GeoJSON areas of interest and ESA
//! acquisition plans (KML).

use crate::fetch;
use crate::model::Model;
use leptos::prelude::*;
use leptos::task::spawn_local;
use orbitui::config;

pub fn open(m: Model, file: web_sys::File) {
    let name = file.name();
    let plan = name.to_ascii_lowercase().ends_with(".kml");
    spawn_local(async move {
        match fetch::file(file).await {
            Ok(text) if plan => m.load_plan(&name, &text),
            Ok(text) => m.load_geojson(&text, &config::file_stem(&name)),
            Err(e) => m.say(e),
        }
    });
}

/// Every file of a file list.
pub fn open_all(m: Model, files: Option<web_sys::FileList>) {
    let Some(files) = files else { return };
    for i in 0..files.length() {
        if let Some(f) = files.get(i) {
            open(m, f);
        }
    }
}

/// Files dropped anywhere on the page.
pub fn accept_drops(m: Model) {
    window_event_listener(leptos::ev::dragover, |e| e.prevent_default());
    window_event_listener(leptos::ev::drop, move |e| {
        e.prevent_default();
        open_all(m, e.data_transfer().and_then(|d| d.files()));
    });
}
