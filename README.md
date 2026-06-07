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

`SolariLighting` is always reset. The `dlss` feature pulls `bevy_anti_alias` with its
`dlss` feature, which depends on `dlss_wgpu` and requires the NVIDIA DLSS SDK at build
time; it only builds on NVIDIA + Vulkan targets and is not built on docs.rs.

## Compatibility

| `big_space_solari` | `big_space` | `bevy_solari` |
|--------------------|-------------|---------------|
| 0.1                | 0.12        | 0.18          |

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
