import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

interface MonitorInfo {
  id: string;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  is_primary: boolean;
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

// DOM elements
let dimensionsEl: HTMLElement;
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

  dimensionsEl = document.getElementById("dimensions")!;
  dragAreaEl = document.getElementById("drag-area")!;

  // Fetch monitors directly
  try {
    monitors = await invoke<MonitorInfo[]>("get_monitors");
    console.log("Monitors loaded:", monitors);
  } catch (err) {
    console.error("Failed to get monitors:", err);
  }

  // Set up native resize on handles
  document.querySelectorAll(".handle").forEach(handle => {
    (handle as HTMLElement).addEventListener("mousedown", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      const handleName = (e.target as HTMLElement).dataset.handle;
      if (handleName && handleToDirection[handleName]) {
        await currentWindow.startResizeDragging(handleToDirection[handleName]);
        // Update after resize completes
        updateDisplay();
        emitRegionUpdate();
      }
    });
  });

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
    updateDisplay();
    throttledEmitRegionUpdate();
  });

  // Initial update
  updateDisplay();
  emitRegionUpdate();
});

async function updateDisplay(): Promise<void> {
  const size = await currentWindow.innerSize();
  const recordWidth = size.width - (BORDER_WIDTH * 2);
  const recordHeight = size.height - (BORDER_WIDTH * 2);
  dimensionsEl.textContent = `${recordWidth} × ${recordHeight}`;
}

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
  const pos = await currentWindow.outerPosition();
  const size = await currentWindow.innerSize();

  // The actual recording area is inside the border
  const recordX = pos.x + BORDER_WIDTH;
  const recordY = pos.y + BORDER_WIDTH;
  const recordWidth = size.width - (BORDER_WIDTH * 2);
  const recordHeight = size.height - (BORDER_WIDTH * 2);

  // Find which monitor the center of the selection is on
  const centerX = recordX + recordWidth / 2;
  const centerY = recordY + recordHeight / 2;

  let monitor = findMonitorAt(centerX, centerY);
  if (!monitor && monitors.length > 0) {
    monitor = monitors[0];
  }

  if (!monitor) {
    console.log("No monitor found for region update");
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

  console.log("Emitting region-updated:", region);
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
    await mainWindow.emit("region-selector-closed", {});
  }
  await currentWindow.close();
}
