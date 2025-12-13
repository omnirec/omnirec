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
    // Fallback to Tauri
    // NOTE: Tauri returns PhysicalPosition/PhysicalSize which are in device pixels
    // We need to convert to logical coordinates to match monitor coordinate system
    const scaleFactor = await currentWindow.scaleFactor();
    const pos = await currentWindow.outerPosition();
    const size = await currentWindow.innerSize();
    
    console.log("Tauri scaleFactor:", scaleFactor);
    console.log("Tauri outerPosition (physical):", pos.x, pos.y);
    console.log("Tauri innerSize (physical):", size.width, "x", size.height);
    
    // Convert from physical to logical coordinates
    windowX = Math.round(pos.x / scaleFactor);
    windowY = Math.round(pos.y / scaleFactor);
    windowWidth = Math.round(size.width / scaleFactor);
    windowHeight = Math.round(size.height / scaleFactor);
    
    console.log("Converted to logical:", windowX, windowY, windowWidth, "x", windowHeight);
  }

  // The actual recording area is inside the border
  // Hyprland returns physical pixels
  // Add 1 extra pixel to ensure the border is completely outside the recording area
  // This accounts for any rounding issues due to scaling
  const borderOffset = BORDER_WIDTH + 1;
  const recordX = windowX + borderOffset;
  const recordY = windowY + borderOffset;
  const recordWidth = windowWidth - (borderOffset * 2);
  const recordHeight = windowHeight - (borderOffset * 2);

  console.log("Record area (physical):", recordX, recordY, recordWidth, "x", recordHeight);

  // Find which monitor the center of the selection is on
  const centerX = recordX + recordWidth / 2;
  const centerY = recordY + recordHeight / 2;

  console.log("Selection window (physical?):", windowX, windowY, windowWidth, "x", windowHeight);
  console.log("Record area after border offset:", recordX, recordY, recordWidth, "x", recordHeight);
  console.log("Looking for monitor containing center point:", centerX, centerY);
  console.log("Available monitors:");
  for (const m of monitors) {
    console.log(`  ${m.name}: origin=(${m.x}, ${m.y}) size=${m.width}x${m.height} scale=${m.scale_factor}`);
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

  console.log("=== REGION CALCULATION ===");
  console.log("Selected monitor:", monitor.name, `(id=${monitor.id})`);
  console.log("Monitor origin:", monitor.x, ",", monitor.y);
  console.log("Monitor size:", monitor.width, "x", monitor.height);
  console.log("Monitor scale_factor:", monitor.scale_factor);
  console.log("Record area (screen coords):", recordX, recordY);
  console.log("Region (monitor-relative):", region.x, ",", region.y, "size:", region.width, "x", region.height);
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
        const pos = await currentWindow.outerPosition();
        const size = await currentWindow.innerSize();
        x = pos.x;
        y = pos.y;
        width = size.width;
        height = size.height;
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
