//! Scale-aware ray-bias override for [`bevy_solari`] at true (large-coordinate) scale.
//!
//! `bevy_solari` hardcodes a room-scale ray interval in its `scene_bindings` shader
//! library: `RAY_T_MIN = 0.001` (1 mm) and `RAY_T_MAX = 100000.0` (100 km). At large
//! render coordinates a single f32 ULP (≈ `|ray_origin| * 2⁻²³`) exceeds the 1 mm
//! floor, so rays self-intersect the surface they originate on — shadow acne and
//! boiling GI — and geometry beyond 100 km is unreachable.
//!
//! [`SolariRayBiasPlugin`] replaces the `bevy_solari::scene_bindings` shader module
//! at runtime with a copy whose `trace_ray` raises the effective near plane in
//! proportion to `length(ray_origin)`, and whose `RAY_T_MAX` is lifted. Near the
//! render origin the proportional term is far below 1 mm, so the near field is
//! byte-identical to stock — this is a pure superset, safe to leave always on.
//!
//! Tune via the [`SolariRayBias`] resource (insert your own before/after adding the
//! plugin, or mutate it at runtime to re-tune live).

use bevy_app::prelude::*;
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_shader::Shader;

/// Canonical embedded asset path of `bevy_solari`'s scene-bindings shader library.
/// Derived from `embedded_asset!` in `bevy_solari/src/scene/mod.rs` (crate name +
/// `src`-stripped subpath). Must match exactly or the override silently no-ops.
const SCENE_BINDINGS_PATH: &str = "embedded://bevy_solari/scene/raytracing_scene_bindings.wgsl";

/// Our replacement module. Keeps `#define_import_path bevy_solari::scene_bindings`
/// so it shadows the original under the same naga_oil module name.
const OVERRIDE_WGSL: &str = include_str!("raytracing_scene_bindings_override.wgsl");

/// The two constant lines in [`OVERRIDE_WGSL`] rewritten from [`SolariRayBias`].
/// Matched as whole lines, so they must stay identical to the shader source.
const T_MAX_LINE: &str = "const RAY_T_MAX = 1000000000.0f;";
const SCALE_LINE: &str = "const RAY_T_MIN_SCALE = 0.000001f;";

/// Tuning for the scale-aware ray bias applied by [`SolariRayBiasPlugin`].
///
/// Mutating this resource at runtime re-applies the override (the shader recompiles),
/// so values can be tuned live.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SolariRayBias {
    /// Effective ray near plane is `max(ray_t_min, length(ray_origin) * scale)`.
    /// Size it to a few f32 ULP: `~8 * 2⁻²³ ≈ 1e-6` clears self-intersection while
    /// staying negligible in the near field.
    pub scale: f32,
    /// Maximum ray distance (render units), replacing the 100 km stock `RAY_T_MAX`.
    pub t_max: f32,
}

impl Default for SolariRayBias {
    fn default() -> Self {
        Self {
            scale: 1.0e-6,
            t_max: 1.0e9,
        }
    }
}

/// Overrides `bevy_solari::scene_bindings` with the scale-aware ray bias.
///
/// Add **after** `SolariPlugins`. Requires the `bevy_solari` shader library to be
/// registered (it is, by `RaytracingScenePlugin`). No-ops on builds without Solari.
pub struct SolariRayBiasPlugin;

impl Plugin for SolariRayBiasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SolariRayBias>();
        app.add_systems(Update, apply_ray_bias_override);
    }
}

#[derive(Default)]
struct BiasState {
    applied: bool,
    frames: u32,
    warned: bool,
}

fn apply_ray_bias_override(
    mut state: Local<BiasState>,
    asset_server: Res<AssetServer>,
    bias: Res<SolariRayBias>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    // Apply once, then only when the tuning resource changes (live re-tune).
    if state.applied && !bias.is_changed() {
        return;
    }

    let Some(handle) = asset_server.get_handle::<Shader>(SCENE_BINDINGS_PATH) else {
        // Library not registered yet (or SolariPlugins absent). Keep polling.
        stall(&mut state, "scene_bindings not registered");
        return;
    };
    let id = handle.id();

    // Overwrite ONLY after the async embedded load has materialised the stock asset.
    // Inserting earlier loses the race: the load lands later and clobbers us.
    if shaders.get(id).is_none() {
        stall(&mut state, "stock shader not loaded yet");
        return;
    }

    let source = build_override_source(&bias);
    if shaders
        .insert(
            id,
            Shader::from_wgsl(source, SCENE_BINDINGS_PATH.to_string()),
        )
        .is_err()
    {
        // Stale asset generation; the slot was freed under us. Retry next frame.
        stall(&mut state, "Assets::insert rejected the shader id");
        return;
    }
    state.applied = true;
    state.frames = 0;
    state.warned = false;
    tracing::info!(
        target: "big_space_solari",
        "SolariRayBiasPlugin applied scale-aware ray bias (scale={}, t_max={}) to `{SCENE_BINDINGS_PATH}`.",
        bias.scale,
        bias.t_max,
    );
}

fn build_override_source(bias: &SolariRayBias) -> String {
    let scale = sanitize(bias.scale, SolariRayBias::default().scale);
    let t_max = sanitize(bias.t_max, SolariRayBias::default().t_max);
    OVERRIDE_WGSL
        .replacen(T_MAX_LINE, &format!("const RAY_T_MAX = {t_max}f;"), 1)
        .replacen(SCALE_LINE, &format!("const RAY_T_MIN_SCALE = {scale}f;"), 1)
}

/// Clamp to a positive, finite value usable as a WGSL literal; fall back otherwise.
fn sanitize(v: f32, fallback: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        fallback
    }
}

fn stall(state: &mut BiasState, reason: &str) {
    state.frames += 1;
    if !state.warned && state.frames > 600 {
        state.warned = true;
        tracing::warn!(
            target: "big_space_solari",
            "SolariRayBiasPlugin could not override `{SCENE_BINDINGS_PATH}` after \
             {} frames ({reason}); scale-aware ray bias is inactive.",
            state.frames,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_keeps_module_path_and_rewrites_both_constants() {
        let src = build_override_source(&SolariRayBias {
            scale: 2.0e-6,
            t_max: 5.0e8,
        });
        // Module identity must survive or Solari's imports fail to resolve.
        assert!(src.starts_with("#define_import_path bevy_solari::scene_bindings"));
        // Both tunable lines were rewritten (template defaults gone, new values in).
        assert!(src.contains("const RAY_T_MAX = 500000000f;"));
        assert!(src.contains("const RAY_T_MIN_SCALE = 0.000002f;"));
        assert!(!src.contains(T_MAX_LINE));
        assert!(!src.contains(SCALE_LINE));
        // trace_ray still funnels every ray (choke point preserved).
        assert!(src.contains("fn trace_ray("));
        assert!(src.contains("length(ray_origin) * RAY_T_MIN_SCALE"));
    }

    #[test]
    fn sanitize_rejects_nonpositive_and_nonfinite() {
        assert_eq!(sanitize(1.0e-6, 9.0), 1.0e-6);
        assert_eq!(sanitize(0.0, 9.0), 9.0);
        assert_eq!(sanitize(-1.0, 9.0), 9.0);
        assert_eq!(sanitize(f32::NAN, 9.0), 9.0);
        assert_eq!(sanitize(f32::INFINITY, 9.0), 9.0);
    }

    #[test]
    fn override_template_lines_match_the_replaced_constants_exactly_once() {
        // `build_override_source` rewrites these whole lines via `replacen(_, 1)`.
        // If a future `bevy_solari` re-sync edits the vendored shader's defaults,
        // the match breaks and the override ships inert. Fail loudly here instead.
        assert_eq!(OVERRIDE_WGSL.matches(T_MAX_LINE).count(), 1);
        assert_eq!(OVERRIDE_WGSL.matches(SCALE_LINE).count(), 1);
    }
}
