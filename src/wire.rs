//! Messages between the app and the forecast worker, as JSON.
//!
//! The worker rebuilds the fleet and the area of interest from these and
//! runs the same prediction as the app would.

use chrono::{DateTime, Utc};
use orbitui::aoi::Area;
use orbitui::catalog::{self, Geometry, Kind, SatInfo};
use orbitui::orbit::{Observer, Sat};
use orbitui::predict::Forecast;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WireSat {
    norad: u64,
    name: String,
    short: String,
    sensor: String,
    swath_km: f64,
    near_km: f64,
    far_km: f64,
    both_sides: bool,
    geometry: Geometry,
    kind: Kind,
    user: bool,
    elements: sgp4::Elements,
}

#[derive(Serialize, Deserialize)]
pub struct WireObserver {
    name: String,
    lat: f64,
    lon: f64,
    alt_km: f64,
    rings: Option<Vec<Vec<(f64, f64)>>>,
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub job: u64,
    sats: Vec<WireSat>,
    observer: WireObserver,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub min_el: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Reply {
    Ready,
    Progress { job: u64, share: f64 },
    Done { job: u64, forecast: Forecast },
}

/// Built-in strings stay static; the user's live as long as the worker.
fn text(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

impl Request {
    pub fn new(job: u64, sats: &[Sat], obs: &Observer, start: DateTime<Utc>, end: DateTime<Utc>, min_el: f64) -> Self {
        let sats = sats
            .iter()
            .map(|s| {
                let i = &s.info;
                WireSat {
                    norad: i.norad,
                    name: i.name.to_string(),
                    short: i.short.to_string(),
                    sensor: i.sensor.to_string(),
                    swath_km: i.swath_km,
                    near_km: i.near_km,
                    far_km: i.far_km,
                    both_sides: i.both_sides,
                    geometry: i.geometry,
                    kind: i.kind,
                    user: i.user,
                    elements: s.elements.clone(),
                }
            })
            .collect();
        let observer = WireObserver {
            name: obs.name.clone(),
            lat: obs.lat,
            lon: obs.lon,
            alt_km: obs.alt_km,
            rings: obs.area.as_ref().map(|a| a.rings.clone()),
        };
        Request { job, sats, observer, start, end, min_el }
    }

    /// The fleet and the area of interest, as the app has them.
    pub fn build(&self) -> (Vec<Sat>, Observer) {
        let builtin = catalog::builtin();
        let sats = self
            .sats
            .iter()
            .filter_map(|w| {
                let info = match builtin.iter().find(|b| b.norad == w.norad && !w.user) {
                    Some(b) => *b,
                    None => SatInfo {
                        norad: w.norad,
                        name: text(&w.name),
                        short: text(&w.short),
                        sensor: text(&w.sensor),
                        swath_km: w.swath_km,
                        near_km: w.near_km,
                        far_km: w.far_km,
                        both_sides: w.both_sides,
                        geometry: w.geometry,
                        kind: w.kind,
                        color: Color::Reset,
                        groups: &[],
                        user: w.user,
                    },
                };
                Sat::new(info, w.elements.clone())
            })
            .collect();
        let o = &self.observer;
        let observer = match o.rings.clone().map(Area::new) {
            Some(Ok(area)) => Observer::with_area(o.name.clone(), area),
            _ => Observer::new(o.name.clone(), o.lat, o.lon, o.alt_km * 1000.0),
        };
        (sats, observer)
    }
}
