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
- A timeline of every opportunity in the forecast (3, 7 or 14 days) against
  night and cloud cover; dragging along it moves the simulated clock.

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
- `src/ui/`: the panels ([Leptos](https://leptos.dev)).
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
