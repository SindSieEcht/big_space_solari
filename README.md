# big_space_solari

Resets [`bevy_solari`](https://crates.io/crates/bevy_solari) (and optionally DLSS)
ray-traced lighting temporal history when a
[`big_space`](https://crates.io/crates/big_space) floating origin recenters.

## Why

When a `big_space` floating origin crosses into a new cell, the rendering origin
shifts and every entity's global transform translates rigidly by the same delta.

That rigid shift is **not** a problem for Bevy's motion-vector prepass: previous
view data and previous mesh transforms are captured together, before propagation, so
the shift cancels in screen space. It **is** a problem for temporal techniques that
key history by absolute world position — ray-traced GI reservoirs, world radiance
caches, and DLSS history — because every stored position jumps by the recenter delta
in a single frame, and last frame's history is rejected as dissimilar, collapsing the
lighting until it re-converges.

This crate detects each recenter and sets `SolariLighting::reset` (and, with the
`dlss` feature, the DLSS Ray Reconstruction reset) for that frame, so the history
restarts cleanly instead of degrading.

## Usage

```rust
use bevy_app::prelude::*;
use big_space_solari::BigSpaceSolariPlugin;

App::new().add_plugins(BigSpaceSolariPlugin);
```

It resets every `SolariLighting` component on any frame a floating origin changes
cell. No configuration.

## Features

| Feature | Effect |
|---------|--------|
| `dlss`  | Also reset DLSS Ray Reconstruction history on recenter |
| `scale-bias` | Adds `SolariRayBiasPlugin` / `SolariRayBias`: override `bevy_solari`'s room-scale ray interval (`RAY_T_MIN` / `RAY_T_MAX`) so ray-traced geometry stays artifact-free at large (floating-origin) coordinates |

`SolariLighting` is always reset. The `dlss` feature pulls `bevy_anti_alias` with its
`dlss` feature, which depends on `dlss_wgpu` and requires the NVIDIA DLSS SDK at build
time; it only builds on NVIDIA + Vulkan targets and is not built on docs.rs.

### Scale-aware ray bias (`scale-bias`)

`bevy_solari` hardcodes a room-scale ray interval (`RAY_T_MIN` = 1 mm, `RAY_T_MAX`
= 100 km). At large render coordinates a single f32 ULP exceeds the 1 mm floor, so
rays self-intersect the surface they originate on (shadow acne, boiling GI) and
geometry past 100 km is unreachable. The `scale-bias` feature adds
`SolariRayBiasPlugin`, which replaces `bevy_solari`'s `scene_bindings` shader at
runtime with a copy that raises the near plane in proportion to `length(ray_origin)`
and lifts `RAY_T_MAX`. Near the origin it is byte-identical to stock, so it is safe
to leave always on.

Add it **after** `SolariPlugins`:

```rust
use bevy_app::prelude::*;
use big_space_solari::{BigSpaceSolariPlugin, SolariRayBiasPlugin};

App::new().add_plugins((BigSpaceSolariPlugin, SolariRayBiasPlugin));
```

Tune via the `SolariRayBias` resource (insert your own before/after adding the
plugin, or mutate it at runtime to re-tune live).

## Compatibility

| `big_space_solari` | `big_space` | `bevy_solari` |
|--------------------|-------------|---------------|
| 0.2                | 0.12        | 0.18          |
| 0.1                | 0.12        | 0.18          |

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
