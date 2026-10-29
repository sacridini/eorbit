//! The Cesium globe (`js/globe.js`) and the geometry Rust computes for it.

use crate::color;
use chrono::{DateTime, Duration, Utc};
use orbitui::astro::{self, MEAN_RADIUS_KM};
use orbitui::orbit::{Observer, Sat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/globe.js")]
extern "C" {
    pub fn init(on_sat: &js_sys::Function, on_ground: &js_sys::Function) -> bool;
    #[wasm_bindgen(js_name = setBase)]
    pub fn set_base(name: &str);
    #[wasm_bindgen(js_name = setFleet)]
    fn set_fleet(json: &str);
    pub fn update(ms: f64, xyz: &[f64]);
    #[wasm_bindgen(js_name = setSelected)]
    pub fn set_selected(index: i32);
    #[wasm_bindgen(js_name = setOrbit)]
    fn set_orbit(xyz: &[f64], track: &[f64], swaths: &str, color: &str);
    #[wasm_bindgen(js_name = clearOrbit)]
    pub fn clear_orbit();
    #[wasm_bindgen(js_name = setHighlight)]
    fn set_highlight(swaths: &str);
    #[wasm_bindgen(js_name = setAoi)]
    fn set_aoi(name: &str, lat: f64, lon: f64, rings: &str);
    #[wasm_bindgen(js_name = flyTo)]
    pub fn fly_to(lat: f64, lon: f64, height: f64, seconds: f64);
    #[wasm_bindgen(js_name = setLabels)]
    pub fn set_labels(on: bool);
    #[wasm_bindgen(js_name = setPicking)]
    pub fn set_picking(on: bool);
    /// Acquired scenes draped over the globe: JSON [{tiles, west, south,
    /// east, north, rings}], with their outline colour.
    #[wasm_bindgen(js_name = setScenes)]
    pub fn set_scenes(json: &str, color: &str);
    /// NASA's daily view over the globe: its tiles' address, "" for none.
    #[wasm_bindgen(js_name = setSky)]
    pub fn set_sky(url: &str);
    #[wasm_bindgen(js_name = flyToBox)]
    pub fn fly_to_box(west: f64, south: f64, east: f64, north: f64);
    #[wasm_bindgen(js_name = setActive)]
    pub fn set_active(on: bool);
}

pub fn fleet(sats: &[Sat]) {
    let list: Vec<_> = sats
        .iter()
        .map(|s| serde_json::json!({ "short": s.info.short, "name": s.info.name, "color": color::css(s.info.color) }))
        .collect();
    set_fleet(&serde_json::Value::from(list).to_string());
}

/// Earth-fixed positions in metres, NaN where the propagation fails.
pub fn positions(sats: &[Sat], t: DateTime<Utc>) -> Vec<f64> {
    let mut xyz = Vec::with_capacity(sats.len() * 3);
    for s in sats {
        match s.state(t) {
            Some(st) => xyz.extend(st.ecef.map(|v| v * 1000.0)),
            None => xyz.extend([f64::NAN; 3]),
        }
    }
    xyz
}

/// Draws the orbit, ground track and swath of `sat` in `[from, to]`.
pub fn orbit(sat: &Sat, from: DateTime<Utc>, to: DateTime<Utc>) {
    let mut xyz = Vec::new();
    let mut t = from;
    while t <= to {
        if let Some(st) = sat.state(t) {
            xyz.extend(st.ecef.map(|v| v * 1000.0));
        }
        t += Duration::seconds(30);
    }
    let track: Vec<f64> = sat.ground_track(from, to, 30).into_iter().flat_map(|(lat, lon)| [lon, lat]).collect();
    set_orbit(&xyz, &track, &swaths(sat, from, to, 30), &color::css(sat.info.color));
}

/// The swath of `sat` around one overflight.
pub fn highlight(sat: Option<&Sat>, t: DateTime<Utc>) {
    let json = match sat {
        Some(s) => swaths(s, t - Duration::minutes(3), t + Duration::minutes(3), 10),
        None => "[]".into(),
    };
    set_highlight(&json);
}

/// Swaths as corridors: the centre line of each band and its width.
fn swaths(sat: &Sat, from: DateTime<Utc>, to: DateTime<Utc>, step_s: i64) -> String {
    let info = &sat.info;
    if !info.images() {
        return "[]".into();
    }
    let track = sat.ground_track(from, to, step_s);
    let width_m = (info.far_km - info.near_km) * 1000.0;
    let middle = (info.near_km + info.far_km) / 2.0;
    let offsets: &[f64] = if info.both_sides { &[middle, -middle] } else { &[middle] };
    let css = color::css(info.color);
    let bands: Vec<_> = offsets
        .iter()
        .map(|&off| {
            let pts: Vec<f64> = track
                .windows(2)
                .flat_map(|w| {
                    let ((lat, lon), (lat2, lon2)) = (w[0], w[1]);
                    let heading = astro::bearing(lat, lon, lat2, lon2);
                    let side = if off >= 0.0 { 90.0 } else { -90.0 };
                    let (plat, plon) = astro::destination(lat, lon, heading + side, off.abs() / MEAN_RADIUS_KM);
                    [plon, plat]
                })
                .collect();
            serde_json::json!({ "pts": pts, "width": width_m, "color": css })
        })
        .collect();
    serde_json::Value::from(bands).to_string()
}

pub fn aoi(obs: &Observer) {
    let rings: Vec<Vec<f64>> = obs
        .area
        .as_ref()
        .map(|a| a.rings.iter().map(|r| r.iter().flat_map(|&(lat, lon)| [lon, lat]).collect()).collect())
        .unwrap_or_default();
    set_aoi(&obs.name, obs.lat, obs.lon, &serde_json::Value::from(rings).to_string());
}
