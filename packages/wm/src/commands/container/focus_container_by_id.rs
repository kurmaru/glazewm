use anyhow::Context;
use uuid::Uuid;

use super::set_focused_descendant;
use crate::{traits::CommonGetters, wm_state::WmState};

pub fn focus_container_by_id(
  container_id: &Uuid,
  state: &mut WmState,
) -> anyhow::Result<()> {
  let focus_target = state
    .container_by_id(*container_id)
    .context("No container with given id")?;

  // Set focus to the target container.
  set_focused_descendant(&focus_target, None);

  // Re-evaluate visibility of all tiling windows in the workspace, since
  // focusing an inactive tiling window should make it the active window.
  if let Some(workspace) = focus_target.workspace() {
    state
      .pending_sync
      .queue_containers_to_redraw(workspace.tiling_windows());
  }

  state.pending_sync.queue_focus_change().queue_cursor_jump();

  Ok(())
}
