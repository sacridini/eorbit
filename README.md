# orbitglobe

When will Sentinel, Landsat, CBERS, MODIS, VIIRS and other Earth observation
missions image your area? orbitglobe answers on a 3D globe, in the browser:
<https://sacridini.github.io/orbitglobe/>.

It is the graphical sibling of [orbitui](https://github.com/sacridini/orbitui),
the terminal planner, and uses its library for orbits, predictions, the
mission catalogue, clouds and place search.

## What it shows

- The fleet in real time on the globe, with the night side shaded; for the
  selected satellite its orbit, ground track and swath.
- The area of interest: search a place, type `lat, lon`, pick it on the globe,
  or load (or drop) a GeoJSON polygon.
- Upcoming imaging opportunities: off-nadir angle, share of the area imaged,
  forecast cloud cover. Opening one takes the clock to it, highlights its
  swath and plots the satellite's path across the area's sky.
- A quality score (0 to 100) for each opportunity: clear sky, Sun height for
  optical sensors, viewing angle and share of the area imaged. The list can be
  sorted by it, and the detail explains it, with a local map of the swath over
  the area.
- A strip of every opportunity in the forecast (3, 7, 14 or 30 days) against
  night and cloud cover under the globe; dragging along it moves the
  simulated clock. Escape or "Reset view" deselects the satellite and returns
  to the area.

Three more views share the screen with the opportunities:

- Timeline: zoomable from 6 hours to 14 days, with the Sun's height as a sky
  band, the forecast cloud cover in total and by layer (low, middle, high),
  and a row per satellite whose bars grow with the score.
- Calendar: days by hours, tinted by the sky over the area, each hour holding
  its best opportunity, with the best chance of the forecast on top.
- Revisit: how often each satellite (and the fleet, and the good images
  alone) images the area, its longest wait and the gaps on a strip.

Opportunities export to a spreadsheet (CSV) or a calendar (ICS). ESA's
acquisition plans for Sentinel-1 and Sentinel-2 (KML, loaded or dropped on the
page) tell whether each pass is planned. The forecast runs in a Web Worker, so
the page stays responsive while it computes.

## Build

Requires Rust with the `wasm32-unknown-unknown` target and
[Trunk](https://trunkrs.dev):

```sh
trunk serve          # http://127.0.0.1:8080
trunk build --release
```

## Layout

- `src/model.rs`: state, downloads (Celestrak elements, Open-Meteo clouds and
  places) and the per-frame loop.
- `src/bin/worker.rs`, `src/wire.rs`: the forecast's Web Worker and its
  messages.
- `src/quality.rs`: the score; `src/sky.rs`: the cloud layers.
- `src/ui/`: the panels and views ([Leptos](https://leptos.dev)).
- `src/globe.rs`: positions, orbits and swaths computed in Rust for the globe.
- `js/globe.js`: draws them on [CesiumJS](https://cesium.com/platform/cesiumjs/).

## Data

- Orbital elements: Celestrak GP data (OMM), with orbitui's bundled snapshot.
- Cloud forecast and place names: [Open-Meteo.com](https://open-meteo.com)
  (CC BY 4.0), with [GeoNames](https://www.geonames.org) for places.
- Base maps: [Sentinel-2 cloudless](https://s2maps.eu) by EOX IT Services GmbH
  (CC BY-NC-SA 4.0, contains modified Copernicus Sentinel data 2024) and
  Esri's World Dark Gray Base.

## License

MIT
