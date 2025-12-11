# Design: Linux Highlight Implementation

## Context

Linux (Wayland) requires a different approach to rendering overlay windows compared to Windows (Win32 layered windows) and macOS (NSWindow). On Wayland, applications cannot arbitrarily position windows or render overlays without compositor cooperation. The standard protocol for overlay surfaces is `wlr-layer-shell`, which is supported by most wlroots-based compositors (Hyprland, Sway, river, etc.) and some others.

### Constraints

- Must work on Wayland compositors supporting `wlr-layer-shell`
- Cannot use X11-specific approaches (XComposite, etc.)
- Must be non-interactive (click-through)
- Must auto-dismiss after ~800ms (matching other platforms)
- Must support multi-monitor setups with correct positioning
- Should not depend on compositor-specific commands (like `hyprctl`)

## Goals / Non-Goals

**Goals:**
- Implement visual highlight matching Windows/macOS appearance (blue border, 800ms duration)
- Use standard Wayland protocols for broad compositor support
- Keep implementation maintainable with reasonable dependencies

**Non-Goals:**
- Fade animation (keep it simple - show then hide after timeout)
- Supporting X11 fallback
- Supporting compositors without layer-shell (graceful no-op)

## Decisions

### Decision: Use wlr-layer-shell Protocol via wayland-client

**Rationale:** The `wlr-layer-shell` protocol is the standard way to create overlay surfaces on Wayland. It is supported by:
- Hyprland
- Sway
- river
- wayfire
- labwc
- And other wlroots-based compositors

This approach avoids compositor-specific commands and will work across the Wayland ecosystem.

**Approach:**
1. Connect to the Wayland display using `wayland-client`
2. Bind to `zwlr_layer_shell_v1` global
3. Create a layer surface on the `overlay` layer
4. Configure as:
   - Anchored to specific output (monitor)
   - Exclusive zone of -1 (no exclusive area, allows click-through)
   - Keyboard interactivity: none
   - Size matching the target area
5. Attach a buffer with the border graphic (rendered via software)
6. Commit and display for 800ms, then destroy

**Alternatives considered:**
1. **Hyprland-specific rules via hyprctl**: Works but locks us to Hyprland only
2. **Tauri WebView window**: Requires compositor-specific rules to behave correctly
3. **GTK4 with layer-shell**: Higher-level but adds GTK dependency

### Decision: Software Rendering for Border

Use CPU rendering to create a simple ARGB buffer with the border. This avoids EGL/Vulkan complexity for a simple rectangle border that only needs to be drawn once per highlight.

### Decision: Spawn Dedicated Thread

Match the Windows/macOS pattern of spawning a thread to manage the highlight lifecycle. The thread will:
1. Initialize Wayland connection
2. Create and configure layer surface
3. Render border to buffer
4. Display for 800ms
5. Clean up and exit

This keeps the Wayland event loop isolated from the main application.

## Implementation Approach

```
┌─────────────────────────────────────────────────────────────┐
│                    show_highlight()                          │
├─────────────────────────────────────────────────────────────┤
│  1. Spawn background thread                                  │
│  2. Connect to Wayland display                              │
│  3. Get wlr_layer_shell global                              │
│  4. Create layer surface on overlay layer                   │
│  5. Configure: position, size, keyboard_interactivity=none  │
│  6. Create shared memory buffer with border graphic         │
│  7. Attach buffer and commit                                │
│  8. Run event loop for 800ms                                │
│  9. Destroy surface and disconnect                          │
└─────────────────────────────────────────────────────────────┘
```

### Layer Surface Configuration

```rust
// Pseudo-code for layer surface setup
layer_surface.set_size(width, height);
layer_surface.set_anchor(ANCHOR_TOP | ANCHOR_LEFT);
layer_surface.set_margin(y, 0, 0, x);  // Position via margins from anchor
layer_surface.set_exclusive_zone(-1);   // Don't reserve space
layer_surface.set_keyboard_interactivity(NONE);
layer_surface.set_layer(OVERLAY);       // Above all windows
```

### Buffer Format

- ARGB8888 format (standard Wayland SHM format)
- Border pixels set to #2196F3 with full alpha
- Interior pixels set to fully transparent (alpha = 0)
- 6-8 pixel border width

### Visual Style (matching other platforms)

- Border color: #2196F3 (blue)
- Border width: 6-8px
- Duration: 800ms
- No fade animation (simple show/hide)
- Corner radius: 0px (simpler for software rendering; could add later)

## Dependencies

Add to `Cargo.toml` under Linux dependencies:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
wayland-client = "0.31"
wayland-protocols-wlr = { version = "0.3", features = ["client"] }
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Compositor doesn't support layer-shell | Check for global availability, log warning and no-op gracefully |
| Multi-monitor positioning complexity | Use output name from monitor enumeration to find correct wl_output |
| Thread safety with Wayland objects | Keep all Wayland interaction within single thread |
| SHM buffer allocation failure | Fall back to no-op with warning |

## Graceful Degradation

If `zwlr_layer_shell_v1` is not available:
1. Log a warning: "Highlight not available: compositor does not support wlr-layer-shell"
2. Return without error (highlight is a nice-to-have, not critical)

## Open Questions

1. **Output selection**: How to map our `MonitorInfo.id` (Hyprland name like "DP-1") to the correct `wl_output`? 
   - Answer: Query output names via `xdg_output_v1` protocol or match by geometry.
