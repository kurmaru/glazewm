use anyhow::Context;
use wm_common::{TilingDirection, WmEvent};

use crate::{
  models::Container,
  traits::{CommonGetters, TilingDirectionGetters},
  user_config::UserConfig,
  wm_state::WmState,
};

/// Toggles the tiling direction of the container's workspace.
///
/// The single-window layout does not arrange tiling windows by tiling
/// direction, but the workspace direction is still tracked so that the
/// command and its event keep working. No split containers are created.
pub fn toggle_tiling_direction(
  container: &Container,
  state: &mut WmState,
  _config: &UserConfig,
) -> anyhow::Result<()> {
  // Only workspaces and tiling windows have a meaningful tiling direction.
  if !(container.is_workspace() || container.is_tiling_window()) {
    return Ok(());
  }

  let workspace = container
    .workspace()
    .context("Container has no workspace.")?;

  workspace.set_tiling_direction(workspace.tiling_direction().inverse());

  state.emit_event(WmEvent::TilingDirectionChanged {
    direction_container: workspace.to_dto()?,
    new_tiling_direction: workspace.tiling_direction(),
  });

  Ok(())
}

/// Sets the tiling direction of the container's workspace.
pub fn set_tiling_direction(
  container: &Container,
  state: &mut WmState,
  config: &UserConfig,
  tiling_direction: &TilingDirection,
) -> anyhow::Result<()> {
  let workspace = container
    .workspace()
    .context("Container has no workspace.")?;

  if workspace.tiling_direction() == *tiling_direction {
    Ok(())
  } else {
    toggle_tiling_direction(container, state, config)
  }
}
