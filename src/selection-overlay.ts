import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface MonitorInfo {
  id: string;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  is_primary: boolean;
  scale_factor: number;
}

interface CaptureRegion {
  monitor_id: string;
  monitor_name: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

// Tauri resize direction type
type ResizeDirection = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

// Constants
const BORDER_WIDTH = 3; // Must match CSS --border-width

// State
let monitors: MonitorInfo[] = [];
let currentWindow: Awaited<ReturnType<typeof getCurrentWindow>>;
let emitTimeout: number | null = null;
let isHyprland = false;

// DOM elements
let dragAreaEl: HTMLElement;

// Map handle names to Tauri resize directions
const handleToDirection: Record<string, ResizeDirection> = {
  "nw": "NorthWest",
  "n": "North",
  "ne": "NorthEast",
  "e": "East",
  "se": "SouthEast",
  "s": "South",
  "sw": "SouthWest",
  "w": "West",
};

// Initialize
window.addEventListener("DOMContentLoaded", async () => {
  console.log("Selection overlay loaded");

  currentWindow = getCurrentWindow();

  dragAreaEl = document.getElementById("drag-area")!;

  // Check if running on Hyprland
  try {
    isHyprland = await invoke<boolean>("is_hyprland");
    console.log("Is Hyprland:", isHyprland);
  } catch (err) {
    console.error("Failed to check Hyprland:", err);
    isHyprland = false;
  }

  console.log("isHyprland result:", isHyprland);

  // On non-Hyprland platforms, add resize handles (they don't work on Hyprland)
  if (!isHyprland) {
    console.log("Adding resize handles for non-Hyprland platform");
    createResizeHandles();
  } else {
    console.log("Skipping resize handles on Hyprland");
    document.body.classList.add("hyprland");
  }

  // Fetch monitors directly
  try {
    monitors = await invoke<MonitorInfo[]>("get_monitors");
    console.log("Monitors loaded:", monitors);
  } catch (err) {
    console.error("Failed to get monitors:", err);
  }

  // Set up native resize on handles (only used on non-Hyprland)
  if (!isHyprland) {
    document.querySelectorAll(".handle").forEach(handle => {
      (handle as HTMLElement).addEventListener("mousedown", async (e) => {
        e.preventDefault();
        e.stopPropagation();
        const handleName = (e.target as HTMLElement).dataset.handle;
        if (handleName && handleToDirection[handleName]) {
          await currentWindow.startResizeDragging(handleToDirection[handleName]);
          // Update after resize completes
          emitRegionUpdate();
        }
      });
    });
  }

  // Native drag on drag area
  dragAreaEl.addEventListener("mousedown", async (e) => {
    e.preventDefault();
    await currentWindow.startDragging();
    // Update after drag completes
    emitRegionUpdate();
  });

  // Close on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeOverlay();
    }
  });

  // Listen for window move/resize events to update display (throttled)
  currentWindow.onMoved(() => {
    throttledEmitRegionUpdate();
  });

  currentWindow.onResized(() => {
    throttledEmitRegionUpdate();
  });

  // Listen for recording start/stop events to hide/show UI elements
  listen("recording-started", () => {
    console.log("Recording started - hiding UI elements");
    document.body.classList.add("recording");
  });

  listen("recording-stopped", () => {
    console.log("Recording stopped - showing UI elements");
    document.body.classList.remove("recording");
  });

  // Initial update
  emitRegionUpdate();
});

// Throttle region updates to avoid spamming events
function throttledEmitRegionUpdate(): void {
  if (emitTimeout !== null) {
    return; // Already scheduled
  }
  emitTimeout = window.setTimeout(() => {
    emitTimeout = null;
    emitRegionUpdate();
  }, 50); // Max 20 updates per second
}

async function emitRegionUpdate(): Promise<void> {
  // On Wayland, Tauri's outerPosition() returns (0,0) - it doesn't work
  // Instead, query Hyprland directly for the window position
  let windowX: number;
  let windowY: number;
  let windowWidth: number;
  let windowHeight: number;
  
  try {
    // Get position from Hyprland (returns physical pixels)
    const [x, y, w, h] = await invoke<[number, number, number, number]>("get_region_selector_position");
    windowX = x;
    windowY = y;
    windowWidth = w;
    windowHeight = h;
    console.log("Position from Hyprland:", windowX, windowY, windowWidth, "x", windowHeight);
  } catch (e) {
    console.error("Failed to get position from Hyprland:", e);
    // Fallback to Tauri (Windows/macOS)
    // Tauri returns PhysicalPosition/PhysicalSize - convert to logical coordinates
    // to match the monitor coordinate system (which is also in logical coordinates)
    const scaleFactor = await currentWindow.scaleFactor();
    const pos = await currentWindow.outerPosition();
    const outerSize = await currentWindow.outerSize();
    const innerSize = await currentWindow.innerSize();
    
    console.log("Tauri scaleFactor:", scaleFactor);
    console.log("Tauri outerPosition (physical):", pos.x, pos.y);
    console.log("Tauri outerSize (physical):", outerSize.width, "x", outerSize.height);
    console.log("Tauri innerSize (physical):", innerSize.width, "x", innerSize.height);
    
    // On Windows, transparent windows have an invisible DWM frame.
    // outerPosition is the top-left of this frame, but we need the content area position.
    // The frame is symmetric left/right, but NOT top/bottom:
    // - Left/Right: equal padding (shadow on both sides)
    // - Top: no padding (content starts at outer top)
    // - Bottom: all the vertical padding (shadow only at bottom)
    const frameLeftPhysical = Math.round((outerSize.width - innerSize.width) / 2);
    const frameTopPhysical = 0;  // No top frame on Windows transparent windows
    
    // Content area position in physical pixels
    const contentX = pos.x + frameLeftPhysical;
    const contentY = pos.y + frameTopPhysical;
    
    console.log("Frame offset (physical):", frameLeftPhysical, "x", frameTopPhysical);
    console.log("Content position (physical):", contentX, contentY);
    
    // Convert to logical coordinates
    windowX = Math.round(contentX / scaleFactor);
    windowY = Math.round(contentY / scaleFactor);
    windowWidth = Math.round(innerSize.width / scaleFactor);
    windowHeight = Math.round(innerSize.height / scaleFactor);
    
    console.log("Content area (logical):", windowX, windowY, windowWidth, "x", windowHeight);
  }

  // The actual recording area is inside the border
  // BORDER_WIDTH is in CSS/logical pixels, add 2 extra pixels for safety
  // At fractional DPI scales (e.g., 150%), rounding errors can cause 1px of border to appear
  const borderOffset = BORDER_WIDTH + 2;
  const recordX = windowX + borderOffset;
  const recordY = windowY + borderOffset;
  const recordWidth = windowWidth - (borderOffset * 2);
  const recordHeight = windowHeight - (borderOffset * 2);
  
  console.log("=== REGION CALCULATION (logical coords) ===");
  console.log("Window position:", windowX, windowY);
  console.log("Window size:", windowWidth, "x", windowHeight);
  console.log("Border offset:", borderOffset);
  console.log("Record area:", recordX, recordY, recordWidth, "x", recordHeight);

  // Find which monitor the center of the selection is on
  const centerX = recordX + recordWidth / 2;
  const centerY = recordY + recordHeight / 2;

  console.log("Center point:", centerX, centerY);
  console.log("Available monitors:");
  for (const m of monitors) {
    console.log(`  ${m.id}: origin=(${m.x}, ${m.y}) size=${m.width}x${m.height} scale=${m.scale_factor}`);
  }

  let monitor = findMonitorAt(centerX, centerY);
  if (!monitor) {
    console.log("No monitor found at center point, trying all monitors...");
    // Debug: show all monitor bounds
    for (const m of monitors) {
      console.log(`  ${m.id}: (${m.x}, ${m.y}) to (${m.x + m.width}, ${m.y + m.height})`);
    }
    if (monitors.length > 0) {
      monitor = monitors[0];
    }
  }

  if (!monitor) {
    console.log("No monitors available!");
    return;
  }

  // Convert to monitor-relative coordinates
  const region: CaptureRegion = {
    monitor_id: monitor.id,
    monitor_name: monitor.name,
    x: recordX - monitor.x,
    y: recordY - monitor.y,
    width: recordWidth,
    height: recordHeight,
  };

  console.log("Selected monitor:", monitor.id, "at", monitor.x, ",", monitor.y);
  console.log("Region (monitor-relative physical):", region.x, ",", region.y, "size:", region.width, "x", region.height);
  console.log("==========================");
  
  // Emit to main window
  const mainWindow = await Window.getByLabel("main");
  if (mainWindow) {
    await mainWindow.emit("region-updated", region);
  }
}

function findMonitorAt(x: number, y: number): MonitorInfo | null {
  for (const monitor of monitors) {
    if (
      x >= monitor.x &&
      x < monitor.x + monitor.width &&
      y >= monitor.y &&
      y < monitor.y + monitor.height
    ) {
      return monitor;
    }
  }
  return null;
}

async function closeOverlay(): Promise<void> {
  const mainWindow = await Window.getByLabel("main");
  if (mainWindow) {
    // Send current geometry with the close event so main window can store it
    // WebviewWindow creation expects logical coordinates, so convert from physical
    try {
      // Try Hyprland IPC first (works on Wayland), fallback to Tauri
      let x: number, y: number, width: number, height: number;
      try {
        const [hx, hy, hw, hh] = await invoke<[number, number, number, number]>("get_region_selector_position");
        x = hx;
        y = hy;
        width = hw;
        height = hh;
      } catch {
        // Tauri returns physical pixels, convert to logical for storage
        const scaleFactor = await currentWindow.scaleFactor();
        const pos = await currentWindow.outerPosition();
        const size = await currentWindow.innerSize();
        x = Math.round(pos.x / scaleFactor);
        y = Math.round(pos.y / scaleFactor);
        width = Math.round(size.width / scaleFactor);
        height = Math.round(size.height / scaleFactor);
        console.log("Storing logical geometry:", { x, y, width, height }, "from physical, scale:", scaleFactor);
      }
      await mainWindow.emit("region-selector-closed", { x, y, width, height });
    } catch (e) {
      console.warn("Failed to get geometry for close event:", e);
      await mainWindow.emit("region-selector-closed", {});
    }
  }
  await currentWindow.close();
}

// Create resize handle elements dynamically (for non-Hyprland platforms)
function createResizeHandles(): void {
  const handlePositions = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
  
  for (const pos of handlePositions) {
    const handle = document.createElement("div");
    handle.className = `handle handle-${pos}`;
    handle.dataset.handle = pos;
    document.body.appendChild(handle);
  }
}
