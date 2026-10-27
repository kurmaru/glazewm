use std::{
  cell::{Ref, RefCell, RefMut},
  collections::VecDeque,
  rc::Rc,
};

use anyhow::Context;
use uuid::Uuid;
use wm_common::{
  ContainerDto, GapsConfig, TilingDirection, WorkspaceConfig, WorkspaceDto,
};
use wm_platform::{Rect, RectDelta};

use crate::{
  impl_common_getters, impl_container_debug,
  impl_tiling_direction_getters,
  models::{
    Container, DirectionContainer, TilingContainer, TilingWindow,
    WindowContainer,
  },
  traits::{CommonGetters, PositionGetters, TilingDirectionGetters},
};

#[derive(Clone)]
pub struct Workspace(Rc<RefCell<WorkspaceInner>>);

#[derive(Debug)]
struct WorkspaceInner {
  id: Uuid,
  parent: Option<Container>,
  children: VecDeque<Container>,
  child_focus_order: VecDeque<Uuid>,
  config: WorkspaceConfig,
  gaps_config: GapsConfig,
  tiling_direction: TilingDirection,
}

impl Workspace {
  pub fn new(
    config: WorkspaceConfig,
    gaps_config: GapsConfig,
    tiling_direction: TilingDirection,
  ) -> Self {
    let workspace = WorkspaceInner {
      id: Uuid::new_v4(),
      parent: None,
      children: VecDeque::new(),
      child_focus_order: VecDeque::new(),
      config,
      gaps_config,
      tiling_direction,
    };

    Self(Rc::new(RefCell::new(workspace)))
  }

  /// Underlying config for the workspace.
  pub fn config(&self) -> WorkspaceConfig {
    self.0.borrow().config.clone()
  }

  /// Update the underlying config for the workspace.
  pub fn set_config(&self, config: WorkspaceConfig) {
    self.0.borrow_mut().config = config;
  }

  /// Whether the workspace is currently displayed by the parent monitor.
  pub fn is_displayed(&self) -> bool {
    self
      .monitor()
      .and_then(|monitor| monitor.displayed_workspace())
      .is_some_and(|workspace| workspace.id() == self.id())
  }

  /// Tiling windows in the workspace's window list order.
  ///
  /// The order follows the container tree from left/top to right/bottom.
  /// In the single-window layout, this is the order that previous/next
  /// window navigation traverses.
  pub fn tiling_windows(&self) -> Vec<TilingWindow> {
    let mut windows = Vec::new();
    collect_tiling_windows(&self.children(), &mut windows);
    windows
  }

  /// Tiling window that currently occupies the workspace.
  ///
  /// This is the most recently focused tiling window. If a floating,
  /// fullscreen, or minimized window has focus, then the previously active
  /// tiling window remains the active tiling window.
  pub fn active_tiling_window(&self) -> Option<TilingWindow> {
    self
      .descendant_focus_order()
      .find_map(|container| container.as_tiling_window().cloned())
  }

  /// Whether the given window is the workspace's active tiling window.
  pub fn is_active_tiling_window(&self, window: &TilingWindow) -> bool {
    self
      .active_tiling_window()
      .is_some_and(|active| active.id() == window.id())
  }

  pub fn set_gaps_config(&self, gaps_config: GapsConfig) {
    self.0.borrow_mut().gaps_config = gaps_config;
  }

  /// Effective outer gaps for this workspace.
  ///
  /// Uses `single_window_outer_gap` when configured because the
  /// single-window layout only ever displays one tiling window at a time,
  /// regardless of how many windows are in the workspace.
  pub fn outer_gaps(&self) -> RectDelta {
    let gaps_config = &self.0.borrow().gaps_config;

    gaps_config
      .single_window_outer_gap
      .as_ref()
      .unwrap_or(&gaps_config.outer_gap)
      .clone()
  }

  /// Gets the bounds of a workspace with the given outer gap config.
  fn workspace_rect_with_gap_config(
    &self,
    outer_gaps: &RectDelta,
  ) -> anyhow::Result<Rect> {
    let monitor =
      self.monitor().context("Workspace has no parent monitor.")?;

    let gaps_config = &self.0.borrow().gaps_config;
    let scale_factor = if gaps_config.scale_with_dpi {
      monitor.native_properties().scale_factor
    } else {
      1.
    };

    // Get the delta between the monitor's bounds and its working area.
    let monitor_bounds = monitor.native_properties().bounds;
    let working_area_delta = monitor
      .native_properties()
      .working_area
      .delta(&monitor_bounds);

    Ok(
      monitor_bounds
        // Scale the gaps if `scale_with_dpi` is enabled. Outer gap config
        // values can be a percentage (relative to the monitor bounds), so
        // the outer gap delta needs to be applied prior to the working
        // area delta.
        .apply_delta(&outer_gaps.inverse(), Some(scale_factor))
        .apply_delta(&working_area_delta, None),
    )
  }

  /// Gets the maximum bounds of a workspace considering both `outer_gap`
  /// and `single_window_outer_gap` config values.
  pub fn max_workspace_rect(&self) -> anyhow::Result<Rect> {
    let gaps_config = &self.0.borrow().gaps_config;

    // Get the workspace rect using `outer_gap`.
    let multi_window_rect =
      self.workspace_rect_with_gap_config(&gaps_config.outer_gap)?;

    let Some(single_gap) = &gaps_config.single_window_outer_gap else {
      return Ok(multi_window_rect);
    };

    // Get the workspace rect using `single_window_outer_gap`.
    let single_window_rect =
      self.workspace_rect_with_gap_config(single_gap)?;

    Ok(multi_window_rect.union(&single_window_rect))
  }

  pub fn to_dto(&self) -> anyhow::Result<ContainerDto> {
    let rect = self.to_rect()?;
    let config = self.config();

    let children = self
      .children()
      .iter()
      .map(CommonGetters::to_dto)
      .try_collect()?;

    let tiling_windows = self.tiling_windows();
    let active_tiling_window = self.active_tiling_window();

    let active_tiling_window_index =
      active_tiling_window.as_ref().and_then(|active| {
        tiling_windows
          .iter()
          .position(|window| window.id() == active.id())
      });

    Ok(ContainerDto::Workspace(WorkspaceDto {
      id: self.id(),
      name: config.name,
      display_name: config.display_name,
      parent_id: self.parent().map(|parent| parent.id()),
      children,
      child_focus_order: self.0.borrow().child_focus_order.clone().into(),
      has_focus: self.has_focus(None),
      is_displayed: self.is_displayed(),
      width: rect.width(),
      height: rect.height(),
      x: rect.x(),
      y: rect.y(),
      tiling_direction: self.tiling_direction(),
      tiling_window_count: tiling_windows.len(),
      tiling_window_ids: tiling_windows
        .iter()
        .map(CommonGetters::id)
        .collect(),
      active_tiling_window_index,
      active_tiling_window_id: active_tiling_window
        .map(|window| window.id()),
    }))
  }
}

impl_container_debug!(Workspace);
impl_common_getters!(Workspace);
impl_tiling_direction_getters!(Workspace);

impl PositionGetters for Workspace {
  fn to_rect(&self) -> anyhow::Result<Rect> {
    self.workspace_rect_with_gap_config(&self.outer_gaps())
  }
}

/// Recursively collects tiling windows in tree order.
///
/// Split containers should not normally be created in the single-window
/// layout, but existing split containers are still traversed in order.
fn collect_tiling_windows(
  containers: &VecDeque<Container>,
  windows: &mut Vec<TilingWindow>,
) {
  for container in containers {
    match container {
      Container::TilingWindow(window) => windows.push(window.clone()),
      Container::Split(split) => {
        collect_tiling_windows(&split.children(), windows);
      }
      _ => {}
    }
  }
}

impl std::fmt::Display for Workspace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Workspace(name={}, tiling_direction={:?})",
      self.config().name,
      self.tiling_direction(),
    )
  }
}
