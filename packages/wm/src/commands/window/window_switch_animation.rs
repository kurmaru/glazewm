use std::{
  thread,
  time::{Duration, Instant},
};

use wm_common::DisplayState;
use wm_platform::{
  NativeWindowWindowsExt, Rect, WindowZOrder, SWP_ASYNCWINDOWPOS,
  SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOSENDCHANGING,
};

use crate::{
  models::{TilingWindow, Workspace},
  traits::{CommonGetters, PositionGetters, WindowGetters},
  wm_state::WmState,
};

/// Duration of the horizontal window-switch animation.
const ANIMATION_DURATION: Duration = Duration::from_millis(180);

/// Delay between animation frames.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Animates a change in the active tiling window for all displayed
/// workspaces.
///
/// The outgoing window slides out to one side while the incoming window
/// slides in from the other. Moving forward (next window) slides to the
/// left; moving backward slides to the right.
pub fn animate_pending_window_switch(state: &WmState) {
  for monitor in state.monitors() {
    let Some(workspace) = monitor.displayed_workspace() else {
      continue;
    };

    let Some(incoming) = workspace.active_tiling_window() else {
      continue;
    };

    let Some(outgoing) =
      state.prev_active_tiling_windows.get(&workspace.id())
    else {
      continue;
    };

    if outgoing.id() == incoming.id()
      || outgoing
        .workspace()
        .is_none_or(|ws| ws.id() != workspace.id())
    {
      continue;
    }

    // Only animate when the previous window was actually visible. This
    // avoids animating windows that were hidden on a workspace that just
    // became displayed.
    if !matches!(
      outgoing.display_state(),
      DisplayState::Shown | DisplayState::Showing
    ) {
      continue;
    }

    // The incoming window must currently be hidden. This skips new
    // windows, which are initially visible before their first redraw.
    if !matches!(
      incoming.display_state(),
      DisplayState::Hidden | DisplayState::Hiding
    ) {
      continue;
    }

    let windows = workspace.tiling_windows();

    if windows.len() < 2 {
      continue;
    }

    let Some(previous_index) = windows
      .iter()
      .position(|window| window.id() == outgoing.id())
    else {
      continue;
    };

    let Some(current_index) = windows
      .iter()
      .position(|window| window.id() == incoming.id())
    else {
      continue;
    };

    let offset =
      switch_offset(previous_index, current_index, windows.len());

    if offset == 0 {
      continue;
    }

    if let Err(err) =
      animate_window_switch(&workspace, outgoing, &incoming, offset)
    {
      tracing::warn!("Failed to animate window switch: {err}");
    }
  }
}

/// Slides the outgoing window off-screen and the incoming window in.
fn animate_window_switch(
  workspace: &Workspace,
  outgoing: &TilingWindow,
  incoming: &TilingWindow,
  offset: isize,
) -> anyhow::Result<()> {
  let workspace_rect = workspace.to_rect()?;

  if workspace_rect.width() <= 0 {
    return Ok(());
  }

  let outgoing_rect = animated_rect(outgoing)?;
  let incoming_rect = animated_rect(incoming)?;

  let (outgoing_end_x, incoming_start_x) =
    slide_positions(&workspace_rect, offset);

  let incoming_start = incoming_rect
    .translate_to_coordinates(incoming_start_x, incoming_rect.y());

  // Position the incoming window before uncloaking it so that it doesn't
  // briefly flash at its final position.
  set_window_rect(incoming, &incoming_start)?;
  show_window(incoming)?;
  set_window_rect(outgoing, &outgoing_rect)?;

  let start = Instant::now();

  loop {
    let progress = (start.elapsed().as_secs_f32()
      / ANIMATION_DURATION.as_secs_f32())
    .min(1.0);
    let eased = ease_out_cubic(progress);

    let outgoing_x = lerp_i32(outgoing_rect.x(), outgoing_end_x, eased);
    let incoming_x = lerp_i32(incoming_start_x, incoming_rect.x(), eased);

    set_window_rect(
      outgoing,
      &outgoing_rect
        .translate_to_coordinates(outgoing_x, outgoing_rect.y()),
    )?;
    set_window_rect(
      incoming,
      &incoming_rect
        .translate_to_coordinates(incoming_x, incoming_rect.y()),
    )?;

    if progress >= 1.0 {
      break;
    }

    thread::sleep(FRAME_INTERVAL);
  }

  Ok(())
}

/// Gets the final X of the outgoing window and the starting X of the
/// incoming window for a switch direction.
///
/// A positive offset slides left (next window); a negative offset slides
/// right (previous window).
fn slide_positions(workspace_rect: &Rect, offset: isize) -> (i32, i32) {
  if offset > 0 {
    (
      workspace_rect.left - workspace_rect.width(),
      workspace_rect.right,
    )
  } else {
    (
      workspace_rect.right,
      workspace_rect.left - workspace_rect.width(),
    )
  }
}

/// Gets the direction to slide based on the old and new positions in the
/// workspace's window list.
fn switch_offset(
  previous_index: usize,
  current_index: usize,
  len: usize,
) -> isize {
  if len < 2 || previous_index == current_index {
    return 0;
  }

  if previous_index == len - 1 && current_index == 0 {
    1
  } else if previous_index == 0 && current_index == len - 1 {
    -1
  } else if current_index > previous_index {
    1
  } else {
    -1
  }
}

/// Gets the outer frame rect used for positioning during the animation.
fn animated_rect(window: &TilingWindow) -> anyhow::Result<Rect> {
  Ok(
    window
      .to_rect()?
      .apply_delta(&window.total_border_delta()?, None),
  )
}

/// Moves and resizes a window without activating it.
fn set_window_rect(
  window: &TilingWindow,
  rect: &Rect,
) -> anyhow::Result<()> {
  window.native().set_window_pos(
    &WindowZOrder::Normal,
    rect,
    SWP_NOACTIVATE
      | SWP_NOCOPYBITS
      | SWP_NOSENDCHANGING
      | SWP_ASYNCWINDOWPOS,
  )?;

  Ok(())
}

/// Uncloaks and shows a window so that it can be animated into view.
fn show_window(window: &TilingWindow) -> anyhow::Result<()> {
  window.native().set_cloaked(false)?;
  window.native().show()?;
  Ok(())
}

/// Cubic ease-out curve.
fn ease_out_cubic(progress: f32) -> f32 {
  let inverse = 1.0 - progress;
  1.0 - inverse * inverse * inverse
}

/// Linearly interpolates between two integer values.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn lerp_i32(start: i32, end: i32, progress: f32) -> i32 {
  (start as f32 + (end - start) as f32 * progress).round() as i32
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn switch_offset_moves_forward_and_backward() {
    assert_eq!(switch_offset(0, 1, 3), 1);
    assert_eq!(switch_offset(1, 0, 3), -1);
    assert_eq!(switch_offset(1, 2, 3), 1);
    assert_eq!(switch_offset(2, 1, 3), -1);
  }

  #[test]
  fn switch_offset_handles_wrapping() {
    assert_eq!(switch_offset(2, 0, 3), 1);
    assert_eq!(switch_offset(0, 2, 3), -1);
  }

  #[test]
  fn switch_offset_is_zero_for_same_or_single_window() {
    assert_eq!(switch_offset(1, 1, 3), 0);
    assert_eq!(switch_offset(0, 0, 1), 0);
  }

  #[test]
  fn slide_positions_depend_on_direction() {
    let workspace = Rect::from_xy(100, 50, 800, 600);

    assert_eq!(slide_positions(&workspace, 1), (100 - 800, 900));
    assert_eq!(slide_positions(&workspace, -1), (900, 100 - 800));
  }

  #[test]
  fn ease_out_cubic_has_expected_endpoints() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
    assert!(ease_out_cubic(0.5) > 0.5);
  }

  #[test]
  fn lerp_hits_both_endpoints() {
    assert_eq!(lerp_i32(10, 810, 0.0), 10);
    assert_eq!(lerp_i32(10, 810, 1.0), 810);
    assert_eq!(lerp_i32(10, 810, 0.5), 410);
  }
}
