# Single-window scrolling layout

This fork replaces GlazeWM's split tiling layout with a Niri-inspired
single-window layout: each workspace keeps an ordered list of tiling
windows, but only the active window occupies the workspace's tiled area.
Inactive tiling windows keep running and are hidden with the configured
`general.hide_method` (`cloak` on Windows).

## Behavior

- New tiling windows are appended to the focused workspace's window list
  and become active automatically.
- The active tiling window always fills the workspace's tiled area.
- `focus --next-window` and `focus --prev-window` switch active windows.
  Navigation wraps around at both ends of the list.
- `focus --direction left/up` behaves like `--prev-window`, and
  `focus --direction right/down` behaves like `--next-window`, because
  tiling windows overlap and have no meaningful geometry.
- Moving a tiling window left/right reorders it in the window list instead
  of creating split panes. Moving past either end wraps around. Vertical
  moves are accepted as aliases but are not bound in the sample config,
  since a single-window list has no vertical geometry.
- `move --direction` no longer moves a tiling window to another
  workspace/monitor at the edge of the list. Use
  `move --workspace-in-direction` or `move --workspace` for that.
- `move --workspace <name>` appends the window to the target workspace.
  The target workspace's active window is preserved.
- Closing or minimizing the active tiling window activates the next most
  recently focused tiling window.
- Switching workspaces restores the workspace's previously focused
  window. Floating, fullscreen, and minimized windows keep their normal
  GlazeWM behavior.

## Configuration

No new configuration keys are required. The sample config adds:

```yaml
- commands: ['focus --prev-window']
  bindings: ['alt+page_up']
- commands: ['focus --next-window']
  bindings: ['alt+page_down']
```

`gaps.single_window_outer_gap` is always used when configured, since only
one tiling window is ever visible.

## IPC API

`query workspaces` and `query monitors` expose these extra fields on every
workspace DTO:

| Field | Description |
|---|---|
| `tilingWindowCount` | Number of tiling windows in the workspace list |
| `tilingWindowIds` | Ordered window IDs, from first to last |
| `activeTilingWindowIndex` | Zero-based active index, or `null` when empty |
| `activeTilingWindowId` | Active window ID, or `null` when empty |

Zebar's built-in `glazewm` provider reads workspaces through
`query monitors`, so `output.glazewm.displayedWorkspace` contains these
fields without any Zebar changes. The theme uses them for the Waybar-style
`● ○ ○` scroll indicator.

## Window switch animation

On Windows, switching the active tiling window now animates the real
windows: the outgoing window slides out while the incoming window slides
in from the opposite side (180ms, cubic ease-out).

This applies whenever an already-displayed workspace changes its active
tiled window, including:

- `focus --next-window` / `focus --prev-window`
- `focus --direction left/right/up/down`
- Manual focus changes such as clicking the taskbar or Alt+Tab

Workspace switches, newly launched windows, and close/minimize do not
animate; those transitions already have no outgoing/incoming pair that can
be animated safely.

The animation runs synchronously in the WM event loop for its duration and
is Windows-only. `hide_method: 'cloak'` is recommended.

## Known limitations

- Tiling splits, stacked panes, and multi-column layouts are intentionally
  unsupported.
- `toggle-tiling-direction` still emits its normal event but only changes
  the workspace's tracked direction; it has no visual effect.
- Resizing a tiling window and all split/tiling-direction commands have no
  visual effect, since the active window always fills the tiled area.
- On macOS, GlazeWM always uses the `place_in_corner` hiding method, so
  inactive windows are moved to a monitor corner rather than cloaked.
  Windows is the primarily supported platform for this layout.
- Drag-and-drop of a window into a tiling area reorders it in the target
  workspace instead of creating a split.
