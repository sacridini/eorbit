//! Cloud cover at the area of interest, in total and by layer, from
//! Open-Meteo (hourly, 16 days, no key; weather data by Open-Meteo.com,
//! CC BY 4.0).

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use orbitui::clouds::Clouds;
use serde::Deserialize;

/// Low, middle and high cloud cover (%), one value per hour; gaps are NaN.
#[derive(Clone, Debug, PartialEq)]
pub struct Layers {
    pub start: DateTime<Utc>,
    pub low: Vec<f64>,
    pub mid: Vec<f64>,
    pub high: Vec<f64>,
}

impl Layers {
    /// The hour holding `t`, as (low, mid, high).
    pub fn at(&self, t: DateTime<Utc>) -> Option<(f64, f64, f64)> {
        let h = (t - self.start).num_minutes().div_euclid(60);
        let i = usize::try_from(h).ok()?;
        Some((*self.low.get(i)?, *self.mid.get(i)?, *self.high.get(i)?))
    }

    pub fn hour(&self, i: usize) -> DateTime<Utc> {
        self.start + Duration::hours(i as i64)
    }
}

pub fn url(lat: f64, lon: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &hourly=cloud_cover,cloud_cover_low,cloud_cover_mid,cloud_cover_high&forecast_days=16&timezone=GMT"
    )
}

#[derive(Deserialize)]
struct Response {
    hourly: Hourly,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<String>,
    cloud_cover: Vec<Option<f64>>,
    cloud_cover_low: Vec<Option<f64>>,
    cloud_cover_mid: Vec<Option<f64>>,
    cloud_cover_high: Vec<Option<f64>>,
}

fn values(v: &[Option<f64>]) -> Vec<f64> {
    v.iter().map(|v| v.unwrap_or(f64::NAN)).collect()
}

pub fn parse(text: &str, at: (f64, f64)) -> Result<(Clouds, Layers), String> {
    let r: Response = serde_json::from_str(text).map_err(|e| format!("Open-Meteo: {e}"))?;
    let first = r.hourly.time.first().ok_or("Open-Meteo: no hourly data")?;
    let start = NaiveDateTime::parse_from_str(first, "%Y-%m-%dT%H:%M").map_err(|e| format!("Open-Meteo time: {e}"))?.and_utc();
    let clouds = Clouds { start, hourly: values(&r.hourly.cloud_cover), at, fetched: Utc::now() };
    let layers = Layers {
        start,
        low: values(&r.hourly.cloud_cover_low),
        mid: values(&r.hourly.cloud_cover_mid),
        high: values(&r.hourly.cloud_cover_high),
    };
    Ok((clouds, layers))
}
