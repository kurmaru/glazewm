use crate::{
  commands::container::set_focused_descendant,
  models::{TilingWindow, WindowContainer, Workspace},
  traits::CommonGetters,
  wm_state::WmState,
};

/// Whether the given window should be shown on its workspace.
///
/// Only the active tiling window is shown on a displayed workspace.
/// Non-tiling windows (floating, fullscreen, and minimized) retain their
/// existing visibility behavior.
pub fn should_show_window(
  window: &WindowContainer,
  workspace: &Workspace,
) -> bool {
  if !workspace.is_displayed() {
    return false;
  }

  match window {
    WindowContainer::TilingWindow(window) => {
      workspace.is_active_tiling_window(window)
    }
    WindowContainer::NonTilingWindow(_) => true,
  }
}

/// Gets the tiling window `offset` positions away from the active tiling
/// window.
///
/// Navigation wraps around at both ends of the workspace's window list.
pub fn adjacent_tiling_window(
  workspace: &Workspace,
  offset: isize,
) -> Option<TilingWindow> {
  let windows = workspace.tiling_windows();

  if windows.is_empty() {
    return None;
  }

  let active_index = workspace
    .active_tiling_window()
    .and_then(|active| {
      windows.iter().position(|window| window.id() == active.id())
    })
    .unwrap_or(0);

  let target_index =
    offset_from_index(active_index, offset, windows.len());

  windows.get(target_index).cloned()
}

/// Focuses the tiling window `offset` positions away from the active
/// tiling window in the given workspace.
///
/// All tiling windows in the workspace are queued for redraw so that the
/// previously active window is hidden and the newly active window is
/// shown.
pub fn focus_tiling_window(
  workspace: &Workspace,
  offset: isize,
  state: &mut WmState,
) {
  let Some(focus_target) = adjacent_tiling_window(workspace, offset)
  else {
    return;
  };

  set_focused_descendant(&focus_target.into(), None);

  state
    .pending_sync
    .queue_focus_change()
    .queue_cursor_jump()
    .queue_containers_to_redraw(workspace.tiling_windows());
}

/// Focuses the next tiling window in the given workspace.
pub fn focus_next_window(workspace: &Workspace, state: &mut WmState) {
  focus_tiling_window(workspace, 1, state);
}

/// Focuses the previous tiling window in the given workspace.
pub fn focus_previous_window(workspace: &Workspace, state: &mut WmState) {
  focus_tiling_window(workspace, -1, state);
}

/// Gets the index reached by moving `offset` positions from `index`,
/// wrapping around a list of the given length.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn offset_from_index(index: usize, offset: isize, len: usize) -> usize {
  let len = len as isize;

  (index as isize + offset).rem_euclid(len) as usize
}

#[cfg(test)]
mod tests {
  use wm_common::ContainerDto;

  use super::*;
  use crate::{
    commands::container::set_focused_descendant,
    models::{Monitor, NonTilingWindow, TilingWindow},
    traits::PositionGetters,
  };

  fn mock_tiling_window(title: &str) -> TilingWindow {
    TilingWindow::mock().title(title.to_string()).call()
  }

  #[test]
  fn tiling_windows_preserve_workspace_order() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");
    let window_3 = mock_tiling_window("3");

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
        window_3.clone().into(),
      ])
      .call();

    let ids = workspace
      .tiling_windows()
      .iter()
      .map(CommonGetters::id)
      .collect::<Vec<_>>();

    assert_eq!(ids, vec![window_1.id(), window_2.id(), window_3.id()]);
  }

  #[test]
  fn active_tiling_window_follows_focus_order() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");
    let window_3 = mock_tiling_window("3");

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
        window_3.clone().into(),
      ])
      .call();

    assert_eq!(
      workspace.active_tiling_window().map(|w| w.id()),
      Some(window_1.id())
    );

    set_focused_descendant(&window_2.clone().into(), None);

    assert_eq!(
      workspace.active_tiling_window().map(|w| w.id()),
      Some(window_2.id())
    );

    set_focused_descendant(&window_3.clone().into(), None);

    assert_eq!(
      workspace.active_tiling_window().map(|w| w.id()),
      Some(window_3.id())
    );
  }

  #[test]
  fn adjacent_window_wraps_in_both_directions() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");
    let window_3 = mock_tiling_window("3");

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
        window_3.clone().into(),
      ])
      .call();

    // The first window is active by default.
    assert_eq!(
      adjacent_tiling_window(&workspace, 1).map(|w| w.id()),
      Some(window_2.id())
    );
    assert_eq!(
      adjacent_tiling_window(&workspace, -1).map(|w| w.id()),
      Some(window_3.id())
    );

    set_focused_descendant(&window_2.clone().into(), None);

    assert_eq!(
      adjacent_tiling_window(&workspace, 1).map(|w| w.id()),
      Some(window_3.id())
    );
    assert_eq!(
      adjacent_tiling_window(&workspace, -1).map(|w| w.id()),
      Some(window_1.id())
    );
  }

  #[test]
  fn adjacent_window_returns_none_for_empty_workspace() {
    let workspace = Workspace::mock().call();

    assert!(adjacent_tiling_window(&workspace, 1).is_none());
  }

  #[test]
  fn only_active_tiling_window_should_be_shown() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");
    let floating_window = NonTilingWindow::mock().call();

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
      ])
      .non_tiling_windows(vec![floating_window.clone()])
      .call();

    let _monitor =
      Monitor::mock().workspaces(vec![workspace.clone()]).call();

    assert!(should_show_window(&window_1.clone().into(), &workspace));
    assert!(!should_show_window(&window_2.clone().into(), &workspace));
    assert!(should_show_window(
      &floating_window.clone().into(),
      &workspace
    ));

    set_focused_descendant(&window_2.clone().into(), None);

    assert!(!should_show_window(&window_1.clone().into(), &workspace));
    assert!(should_show_window(&window_2.clone().into(), &workspace));
  }

  #[test]
  fn windows_on_hidden_workspace_should_not_be_shown() {
    let displayed_window = mock_tiling_window("1");
    let hidden_window = mock_tiling_window("2");

    let displayed_workspace = Workspace::mock()
      .tiling_containers(vec![displayed_window.clone().into()])
      .call();

    let hidden_workspace = Workspace::mock()
      .name("2".to_string())
      .tiling_containers(vec![hidden_window.clone().into()])
      .call();

    let _monitor = Monitor::mock()
      .workspaces(vec![
        displayed_workspace.clone(),
        hidden_workspace.clone(),
      ])
      .call();

    assert!(should_show_window(
      &displayed_window.clone().into(),
      &displayed_workspace
    ));
    assert!(!hidden_workspace.is_displayed());
    assert!(!should_show_window(
      &hidden_window.clone().into(),
      &hidden_workspace
    ));
  }

  #[test]
  fn active_and_inactive_tiling_windows_share_workspace_rect() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
      ])
      .call();

    let _monitor =
      Monitor::mock().workspaces(vec![workspace.clone()]).call();

    let workspace_rect = workspace.to_rect().unwrap();

    assert_eq!(window_1.to_rect().unwrap(), workspace_rect);
    assert_eq!(window_2.to_rect().unwrap(), workspace_rect);
  }

  #[test]
  fn workspace_dto_exports_active_window_index() {
    let window_1 = mock_tiling_window("1");
    let window_2 = mock_tiling_window("2");
    let window_3 = mock_tiling_window("3");

    let workspace = Workspace::mock()
      .tiling_containers(vec![
        window_1.clone().into(),
        window_2.clone().into(),
        window_3.clone().into(),
      ])
      .call();

    let empty_workspace = Workspace::mock().name("2".to_string()).call();

    let _monitor = Monitor::mock()
      .workspaces(vec![workspace.clone(), empty_workspace.clone()])
      .call();

    set_focused_descendant(&window_2.clone().into(), None);

    let ContainerDto::Workspace(workspace_dto) =
      workspace.to_dto().unwrap()
    else {
      panic!("Expected workspace DTO.");
    };

    assert_eq!(workspace_dto.tiling_window_count, 3);
    assert_eq!(
      workspace_dto.tiling_window_ids,
      vec![window_1.id(), window_2.id(), window_3.id()]
    );
    assert_eq!(workspace_dto.active_tiling_window_index, Some(1));
    assert_eq!(workspace_dto.active_tiling_window_id, Some(window_2.id()));

    // The IPC contract uses camelCase field names.
    let workspace_json = serde_json::to_value(&workspace_dto).unwrap();
    assert_eq!(workspace_json["tilingWindowCount"], 3);
    assert_eq!(workspace_json["activeTilingWindowIndex"], 1);
    assert_eq!(
      workspace_json["activeTilingWindowId"],
      window_2.id().to_string()
    );

    let ContainerDto::Workspace(empty_workspace_dto) =
      empty_workspace.to_dto().unwrap()
    else {
      panic!("Expected workspace DTO.");
    };

    assert_eq!(empty_workspace_dto.tiling_window_count, 0);
    assert_eq!(
      empty_workspace_dto.tiling_window_ids,
      Vec::<uuid::Uuid>::new()
    );
    assert_eq!(empty_workspace_dto.active_tiling_window_index, None);
    assert_eq!(empty_workspace_dto.active_tiling_window_id, None);
  }
}
