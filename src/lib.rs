//! Resets [`bevy_solari`] (and optionally DLSS) ray-traced lighting temporal
//! history when a [`big_space`] floating origin recenters.
//!
//! When a `big_space` floating origin crosses into a new cell, the rendering origin
//! shifts and every entity's global transform translates rigidly by the same delta.
//! Bevy's motion-vector prepass captures previous view and previous mesh transforms
//! together, so this rigid shift cancels in screen space and does not corrupt motion
//! vectors. It does, however, invalidate temporal techniques that key history by
//! absolute world position: ray-traced GI reservoirs, world radiance caches, and
//! DLSS history all see every stored position jump by the recenter delta in a single
//! frame, and reject last frame's history as dissimilar.
//!
//! [`BigSpaceSolariPlugin`] detects each recenter and sets
//! [`SolariLighting::reset`](bevy_solari::prelude::SolariLighting::reset) so the
//! history restarts cleanly for the one frame it takes to re-converge. Enable the
//! `dlss` feature to also reset DLSS Ray Reconstruction.

use std::collections::HashMap;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_solari::prelude::SolariLighting;
use big_space::prelude::{BigSpace, BigSpaceSystems, CellCoord};

#[cfg(feature = "dlss")]
use bevy_anti_alias::dlss::{Dlss, DlssRayReconstructionFeature};

/// Resets ray-traced lighting temporal history on each floating-origin recenter.
pub struct BigSpaceSolariPlugin;

impl Plugin for BigSpaceSolariPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            reset_on_recenter.after(BigSpaceSystems::RecenterLargeTransforms),
        );
    }
}

fn reset_on_recenter(
    mut last: Local<HashMap<Entity, CellCoord>>,
    spaces: Query<(Entity, &BigSpace)>,
    origins: Query<&CellCoord>,
    mut solari: Query<&mut SolariLighting>,
    #[cfg(feature = "dlss")] mut dlss: Query<&mut Dlss<DlssRayReconstructionFeature>>,
) {
    let mut recentered = false;
    for (root, space) in &spaces {
        let Some(origin) = space.floating_origin else {
            continue;
        };
        let Ok(cell) = origins.get(origin) else {
            continue;
        };
        if recentered_since_last(&mut last, root, *cell) {
            recentered = true;
        }
    }

    if !recentered {
        return;
    }

    for mut solari in &mut solari {
        solari.reset = true;
    }
    #[cfg(feature = "dlss")]
    for mut dlss in &mut dlss {
        dlss.reset = true;
    }
}

fn recentered_since_last(
    last: &mut HashMap<Entity, CellCoord>,
    root: Entity,
    cell: CellCoord,
) -> bool {
    last.insert(root, cell).is_some_and(|prev| prev != cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_fires_only_on_a_changed_cell() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut last = HashMap::new();
        let origin = CellCoord::new(0, 0, 0);
        let moved = CellCoord::new(0, 1, 0);

        assert!(!recentered_since_last(&mut last, a, origin));
        assert!(!recentered_since_last(&mut last, a, origin));
        assert!(recentered_since_last(&mut last, a, moved));
        assert!(!recentered_since_last(&mut last, b, origin));
    }
}
