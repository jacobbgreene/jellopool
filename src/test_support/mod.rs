//! Small shared test primitives, not a universal game fixture.
//! Feature-specific entities and expectations live with their owning tests.
pub(crate) mod input;
pub(crate) mod performance;
pub(crate) mod ui;

use crate::prelude::*;
use std::time::Duration;

/// Advance a deterministic frame. No TimePlugin or wall-clock sleeps in unit tests.
pub(crate) fn step(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(1.0 / 60.0));
    app.update();
}
