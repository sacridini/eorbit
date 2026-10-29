//! The state in the page's address, after `#`: a link opens the same area,
//! view, satellite and opportunity.
//!
//! `at=lat,lon&name=…&area=…&view=timeline&sat=62261&t=1793300000&days=14`,
//! where `area` holds the polygon's rings as encoded polylines (the format
//! of Google's maps), separated by commas.

use crate::model::View;
use chrono::{DateTime, Utc};
use orbitui::aoi::Area;
use orbitui::orbit::Observer;

/// A shared polygon is thinned to this many vertices per ring.
const MAX_VERTICES: usize = 300;
/// Polylines keep five decimals of a degree (about a metre).
const SCALE: f64 = 1e5;
pub const DEFAULT_DAYS: f64 = 7.0;

/// What a link asks for; what it leaves out stays as it is.
#[derive(Default)]
pub struct Shared {
    pub observer: Option<Observer>,
    pub view: Option<View>,
    pub sat: Option<u64>,
    /// The opportunity of `sat` at that time.
    pub at: Option<DateTime<Utc>>,
    pub days: Option<f64>,
}

pub fn encode(obs: &Observer, view: View, sat: Option<u64>, at: Option<DateTime<Utc>>, days: f64) -> String {
    let mut parts = vec![format!("at={:.5},{:.5}", obs.lat, obs.lon), format!("name={}", component(&obs.name))];
    if let Some(area) = &obs.area {
        let rings: Vec<String> = area.rings.iter().map(|r| polyline(r)).collect();
        parts.push(format!("area={}", component(&rings.join(","))));
    }
    if view != View::Globe {
        parts.push(format!("view={}", view.id()));
    }
    if let Some(n) = sat {
        parts.push(format!("sat={n}"));
    }
    if let Some(t) = at {
        parts.push(format!("t={}", t.timestamp()));
    }
    if days != DEFAULT_DAYS {
        parts.push(format!("days={days}"));
    }
    parts.join("&")
}

pub fn decode(hash: &str) -> Shared {
    let mut shared = Shared::default();
    let (mut point, mut name, mut area) = (None, None, None);
    for pair in hash.trim_start_matches('#').split('&') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        let value = uncomponent(value);
        match key {
            "at" => {
                point = value.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
            }
            "name" => name = Some(value),
            "area" => area = Area::new(value.split(',').map(unpolyline).collect()).ok(),
            "view" => shared.view = View::from_id(&value),
            "sat" => shared.sat = value.parse().ok(),
            "t" => shared.at = value.parse().ok().and_then(|t| DateTime::from_timestamp(t, 0)),
            "days" => shared.days = value.parse().ok().filter(|d: &f64| *d > 0.0 && *d <= 30.0),
            _ => {}
        }
    }
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Shared area".into());
    shared.observer = match (area, point) {
        (Some(area), _) => Some(Observer::with_area(name, area)),
        (None, Some((lat, lon))) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Some(Observer::new(name, lat, lon, 0.0)),
        _ => None,
    };
    shared
}

fn component(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}

fn uncomponent(s: &str) -> String {
    js_sys::decode_uri_component(s).map(String::from).unwrap_or_else(|_| s.to_string())
}

/// A ring of (lat, lon) as an encoded polyline.
fn polyline(ring: &[(f64, f64)]) -> String {
    let step = ring.len().div_ceil(MAX_VERTICES).max(1);
    let mut out = String::new();
    let (mut last_lat, mut last_lon) = (0_i64, 0_i64);
    for &(lat, lon) in ring.iter().step_by(step) {
        let (lat, lon) = ((lat * SCALE).round() as i64, (lon * SCALE).round() as i64);
        push_number(&mut out, lat - last_lat);
        push_number(&mut out, lon - last_lon);
        (last_lat, last_lon) = (lat, lon);
    }
    out
}

fn push_number(out: &mut String, n: i64) {
    let mut v = if n < 0 { !(n << 1) } else { n << 1 };
    while v >= 0x20 {
        out.push(char::from(((0x20 | (v & 0x1f)) + 63) as u8));
        v >>= 5;
    }
    out.push(char::from((v + 63) as u8));
}

/// An encoded polyline back to (lat, lon); what does not decode is dropped.
fn unpolyline(s: &str) -> Vec<(f64, f64)> {
    let mut numbers = Vec::new();
    let mut bytes = s.bytes();
    'numbers: loop {
        let (mut value, mut shift) = (0_i64, 0);
        loop {
            let Some(b) = bytes.next() else { break 'numbers };
            let Some(chunk) = (b as i64).checked_sub(63).filter(|c| (0..64).contains(c)) else { break 'numbers };
            value |= (chunk & 0x1f) << shift;
            shift += 5;
            if chunk < 0x20 {
                break;
            }
            if shift > 60 {
                break 'numbers;
            }
        }
        numbers.push(if value & 1 == 1 { !(value >> 1) } else { value >> 1 });
    }
    let (mut lat, mut lon) = (0_i64, 0_i64);
    numbers
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[dlat, dlon]| {
            lat += dlat;
            lon += dlon;
            (lat as f64 / SCALE, lon as f64 / SCALE)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polylines_round_trip() {
        // Google's documented example.
        let ring = [(38.5, -120.2), (40.7, -120.95), (43.252, -126.453)];
        let text = polyline(&ring);
        assert_eq!(text, "_p~iF~ps|U_ulLnnqC_mqNvxq`@");
        for (a, b) in unpolyline(&text).iter().zip(ring) {
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
        }
    }
}
