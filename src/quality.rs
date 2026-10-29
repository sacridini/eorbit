//! How good an imaging opportunity is likely to be, from 0 to 100.
//!
//! The score multiplies four shares, each between 0 and 1:
//! - clear sky: one minus the forecast cloud cover, for sensors that do not
//!   see through clouds (optical, lidar); unknown beyond the forecast;
//! - sunlight: 0 with the Sun at the horizon, 1 from 25° up, for optical
//!   sensors (the low Sun of winter and high latitudes darkens images);
//! - viewing angle: 1 at nadir, falling to 0.6 at 40° off-nadir and beyond
//!   (oblique views stretch pixels and lengthen shadows);
//! - the share of the area of interest inside the swath.

use orbitui::catalog::Kind;
use orbitui::predict::Acquisition;

#[derive(Clone, Copy, Debug)]
pub struct Factors {
    /// `None` when the cloud forecast does not reach the opportunity.
    pub clear: Option<f64>,
    pub sun: f64,
    pub look: f64,
    pub cover: f64,
}

impl Factors {
    pub fn score(&self) -> f64 {
        100.0 * self.clear.unwrap_or(1.0) * self.sun * self.look * self.cover
    }
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn factors(a: &Acquisition, kind: Kind, cloud: Option<f64>) -> Factors {
    let sees_clouds = matches!(kind, Kind::Optical | Kind::Lidar);
    Factors {
        clear: if sees_clouds { cloud.map(|c| (1.0 - c / 100.0).clamp(0.0, 1.0)) } else { Some(1.0) },
        sun: if kind == Kind::Optical { smoothstep(0.0, 25.0, a.sun_el) } else { 1.0 },
        look: 1.0 - 0.4 * (a.off_nadir / 40.0).clamp(0.0, 1.0),
        cover: a.coverage.clamp(0.0, 1.0),
    }
}

pub fn score(a: &Acquisition, kind: Kind, cloud: Option<f64>) -> f64 {
    factors(a, kind, cloud).score()
}
