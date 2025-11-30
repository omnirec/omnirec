import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";

// Types matching Rust structs
interface WindowInfo {
  handle: number;
  title: string;
  process_name: string;
}

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

type CaptureMode = "window" | "region";
type RecordingState = "idle" | "recording" | "saving";

interface RecordingResult {
  success: boolean;
  file_path: string | null;
  error: string | null;
}

// DOM Elements
let windowListEl: HTMLElement | null;
let windowSelectionEl: HTMLElement | null;
let regionSelectionEl: HTMLElement | null;
let regionDisplayEl: HTMLElement | null;
let recordBtn: HTMLButtonElement | null;
let refreshBtn: HTMLButtonElement | null;
let selectRegionBtn: HTMLButtonElement | null;
let modeWindowBtn: HTMLButtonElement | null;
let modeRegionBtn: HTMLButtonElement | null;
let timerEl: HTMLElement | null;
let statusEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultPathEl: HTMLElement | null;
let openFolderBtn: HTMLButtonElement | null;

// State
let captureMode: CaptureMode = "window";
let selectedWindow: WindowInfo | null = null;
let selectedRegion: CaptureRegion | null = null;
let regionSelectorWindow: WebviewWindow | null = null;
let currentState: RecordingState = "idle";
let timerInterval: number | null = null;
let recordingStartTime: number = 0;

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", () => {
  windowListEl = document.querySelector("#window-list");
  windowSelectionEl = document.querySelector("#window-selection");
  regionSelectionEl = document.querySelector("#region-selection");
  regionDisplayEl = document.querySelector("#region-display");
  recordBtn = document.querySelector("#record-btn");
  refreshBtn = document.querySelector("#refresh-btn");
  selectRegionBtn = document.querySelector("#select-region-btn");
  modeWindowBtn = document.querySelector("#mode-window-btn");
  modeRegionBtn = document.querySelector("#mode-region-btn");
  timerEl = document.querySelector("#timer");
  statusEl = document.querySelector("#status");
  resultEl = document.querySelector("#result");
  resultPathEl = document.querySelector("#result-path");
  openFolderBtn = document.querySelector("#open-folder-btn");

  // Set up event listeners
  refreshBtn?.addEventListener("click", loadWindows);
  recordBtn?.addEventListener("click", handleRecordClick);
  openFolderBtn?.addEventListener("click", handleOpenFolder);
  selectRegionBtn?.addEventListener("click", openRegionSelector);
  modeWindowBtn?.addEventListener("click", () => setCaptureMode("window"));
  modeRegionBtn?.addEventListener("click", () => setCaptureMode("region"));

  // Listen for region updates from selector window (continuous updates as user moves/resizes)
  listen<CaptureRegion>("region-updated", (event) => {
    console.log("Received region-updated:", event.payload);
    selectedRegion = event.payload;
    updateRegionDisplay();
    updateRecordButton();
  });

  // Listen for selector window closed
  listen("region-selector-closed", () => {
    regionSelectorWindow = null;
    if (currentState === "idle") {
      selectedRegion = null;
      updateRegionDisplay();
      updateRecordButton();
    }
  });

  // Initial load
  loadWindows();
});

// Set capture mode
function setCaptureMode(mode: CaptureMode): void {
  if (currentState !== "idle") return;

  captureMode = mode;

  // Update button states
  modeWindowBtn?.classList.toggle("active", mode === "window");
  modeRegionBtn?.classList.toggle("active", mode === "region");

  // Show/hide sections
  windowSelectionEl?.classList.toggle("hidden", mode !== "window");
  regionSelectionEl?.classList.toggle("hidden", mode !== "region");

  // Clear window selection when switching to region mode
  if (mode === "region") {
    selectedWindow = null;
    document.querySelectorAll(".window-item.selected").forEach((el) => {
      el.classList.remove("selected");
    });
  }

  updateRecordButton();
  setStatus(mode === "window" ? "Select a window to record" : "Select a region to record");
}

// Update region display
function updateRegionDisplay(): void {
  if (!regionDisplayEl) return;

  if (selectedRegion) {
    regionDisplayEl.classList.add("has-selection");
    regionDisplayEl.innerHTML = `
      <div class="region-details">
        <div class="region-details__dimensions">${selectedRegion.width} x ${selectedRegion.height}</div>
        <div class="region-details__monitor">${selectedRegion.monitor_name}</div>
      </div>
    `;
  } else {
    regionDisplayEl.classList.remove("has-selection");
    regionDisplayEl.innerHTML = '<p class="region-placeholder">No region selected</p>';
  }
}

// Open region selector - creates a draggable/resizable selection rectangle
async function openRegionSelector(): Promise<void> {
  console.log("openRegionSelector called");

  // If selector already open, just focus it
  if (regionSelectorWindow) {
    await regionSelectorWindow.setFocus();
    return;
  }

  setStatus("Opening region selector...");

  try {
    // Get monitors for coordinate mapping
    console.log("Fetching monitors...");
    const monitors = await invoke<MonitorInfo[]>("get_monitors");
    console.log("Monitors received:", monitors);

    if (monitors.length === 0) {
      setStatus("No monitors found", true);
      return;
    }

    // Find primary monitor or use first one
    const primaryMonitor = monitors.find(m => m.is_primary) || monitors[0];

    // Default selection size and position (centered on primary monitor)
    const defaultWidth = 640;
    const defaultHeight = 480;
    const startX = primaryMonitor.x + Math.floor((primaryMonitor.width - defaultWidth) / 2);
    const startY = primaryMonitor.y + Math.floor((primaryMonitor.height - defaultHeight) / 2);

    // Determine the URL based on environment
    const isDev = window.location.hostname === "localhost";
    const overlayUrl = isDev
      ? "http://localhost:1420/selection-overlay.html"
      : "selection-overlay.html";

    console.log("Creating selector window:", { overlayUrl, startX, startY, defaultWidth, defaultHeight });

    // Minimum size for region (100px recording area + 6px for borders)
    const minSize = 106;

    // Create a small, draggable, resizable window (the selection rectangle itself)
    const selector = new WebviewWindow("region-selector", {
      url: overlayUrl,
      title: "Region Selection",
      decorations: false,
      transparent: true,
      alwaysOnTop: true,
      skipTaskbar: true,
      x: startX,
      y: startY,
      width: defaultWidth,
      height: defaultHeight,
      minWidth: minSize,
      minHeight: minSize,
    });

    // Wait for window creation
    await new Promise<void>((resolve, reject) => {
      selector.once("tauri://created", () => {
        console.log("Selector window created");
        resolve();
      });
      selector.once("tauri://error", (e) => {
        console.error("Failed to create selector window:", e);
        reject(new Error(`Failed to create selector: ${e}`));
      });
    });

    regionSelectorWindow = selector;
    await selector.setFocus();

    // Set initial region immediately so Record button is enabled
    selectedRegion = {
      monitor_id: primaryMonitor.id,
      monitor_name: primaryMonitor.name,
      x: startX - primaryMonitor.x,
      y: startY - primaryMonitor.y,
      width: defaultWidth,
      height: defaultHeight,
    };
    updateRegionDisplay();
    updateRecordButton();

    console.log("Selector ready");
    setStatus("Drag to move, resize corners, click Record when ready");

  } catch (error) {
    console.error("Error opening region selector:", error);
    setStatus(`Error: ${error}`, true);
  }
}

// Load available windows
async function loadWindows(): Promise<void> {
  if (!windowListEl) return;

  windowListEl.innerHTML = '<p class="loading">Loading windows...</p>';
  selectedWindow = null;
  updateRecordButton();

  try {
    const windows = await invoke<WindowInfo[]>("get_windows");

    if (windows.length === 0) {
      windowListEl.innerHTML = '<p class="empty">No capturable windows found</p>';
      return;
    }

    windowListEl.innerHTML = "";
    for (const win of windows) {
      const item = createWindowItem(win);
      windowListEl.appendChild(item);
    }
  } catch (error) {
    windowListEl.innerHTML = `<p class="error">Error loading windows: ${error}</p>`;
    setStatus(`Error: ${error}`, true);
  }
}

// Create a window list item element
function createWindowItem(win: WindowInfo): HTMLElement {
  const item = document.createElement("div");
  item.className = "window-item";
  item.dataset.handle = String(win.handle);

  item.innerHTML = `
    <div class="window-item__title">${escapeHtml(win.title)}</div>
    <div class="window-item__process">${escapeHtml(win.process_name)}</div>
  `;

  item.addEventListener("click", () => selectWindow(win, item));

  return item;
}

// Select a window for recording
function selectWindow(win: WindowInfo, element: HTMLElement): void {
  if (currentState !== "idle") return;

  // Remove selection from previous
  document.querySelectorAll(".window-item.selected").forEach((el) => {
    el.classList.remove("selected");
  });

  // Select new
  element.classList.add("selected");
  selectedWindow = win;
  updateRecordButton();
  setStatus(`Selected: ${win.title}`);
}

// Handle record button click
async function handleRecordClick(): Promise<void> {
  if (currentState === "idle") {
    await startRecording();
  } else if (currentState === "recording") {
    await stopRecording();
  }
}

// Start recording
async function startRecording(): Promise<void> {
  if (captureMode === "window" && !selectedWindow) {
    setStatus("Please select a window first", true);
    return;
  }

  if (captureMode === "region" && !selectedRegion) {
    setStatus("Please select a region first", true);
    return;
  }

  setStatus("Starting recording...");
  disableSelection(true);

  try {
    if (captureMode === "window" && selectedWindow) {
      await invoke("start_recording", { windowHandle: selectedWindow.handle });
    } else if (captureMode === "region" && selectedRegion) {
      console.log("Starting region recording with:", selectedRegion);
      await invoke("start_region_recording", {
        monitorId: selectedRegion.monitor_id,
        x: Math.round(selectedRegion.x),
        y: Math.round(selectedRegion.y),
        width: Math.round(selectedRegion.width),
        height: Math.round(selectedRegion.height),
      });
    }

    currentState = "recording";
    updateRecordButton();
    startTimer();
    setStatus("Recording...");
  } catch (error) {
    setStatus(`Failed to start recording: ${error}`, true);
    disableSelection(false);
  }
}

// Stop recording
async function stopRecording(): Promise<void> {
  setStatus("Stopping recording...");
  currentState = "saving";
  updateRecordButton();
  stopTimer();

  try {
    const result = await invoke<RecordingResult>("stop_recording");

    if (result.success && result.file_path) {
      showResult(result.file_path);
      setStatus("Recording saved successfully!");
    } else {
      setStatus(`Recording failed: ${result.error || "Unknown error"}`, true);
    }
  } catch (error) {
    setStatus(`Error stopping recording: ${error}`, true);
  }

  currentState = "idle";
  updateRecordButton();
  disableSelection(false);
}

// Update record button state
function updateRecordButton(): void {
  if (!recordBtn) return;

  recordBtn.classList.remove("recording", "saving");

  switch (currentState) {
    case "idle":
      recordBtn.textContent = "Record";
      if (captureMode === "window") {
        recordBtn.disabled = !selectedWindow;
      } else {
        recordBtn.disabled = !selectedRegion;
      }
      break;
    case "recording":
      recordBtn.textContent = "Stop";
      recordBtn.disabled = false;
      recordBtn.classList.add("recording");
      break;
    case "saving":
      recordBtn.textContent = "Saving...";
      recordBtn.disabled = true;
      recordBtn.classList.add("saving");
      break;
  }
}

// Timer functions
function startTimer(): void {
  recordingStartTime = Date.now();
  updateTimerDisplay();
  timerInterval = window.setInterval(updateTimerDisplay, 1000);
  timerEl?.classList.add("recording");
}

function stopTimer(): void {
  if (timerInterval !== null) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
  timerEl?.classList.remove("recording");
}

function updateTimerDisplay(): void {
  if (!timerEl) return;

  const elapsed = Math.floor((Date.now() - recordingStartTime) / 1000);
  const minutes = Math.floor(elapsed / 60);
  const seconds = elapsed % 60;
  timerEl.textContent = `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

// Disable/enable selection during recording
function disableSelection(disabled: boolean): void {
  if (refreshBtn) {
    refreshBtn.disabled = disabled;
  }
  if (selectRegionBtn) {
    selectRegionBtn.disabled = disabled;
  }
  if (modeWindowBtn) {
    modeWindowBtn.disabled = disabled;
  }
  if (modeRegionBtn) {
    modeRegionBtn.disabled = disabled;
  }

  document.querySelectorAll(".window-item").forEach((el) => {
    if (disabled) {
      el.classList.add("disabled");
    } else {
      el.classList.remove("disabled");
    }
  });
}

// Show result section
function showResult(filePath: string): void {
  if (!resultEl || !resultPathEl) return;

  resultPathEl.textContent = filePath;
  resultEl.classList.remove("hidden");

  // Store path for open folder button
  resultEl.dataset.path = filePath;
}

// Handle open folder button
async function handleOpenFolder(): Promise<void> {
  const path = resultEl?.dataset.path;
  if (!path) return;

  try {
    // Reveal the file in the containing folder
    await revealItemInDir(path);
  } catch (error) {
    setStatus(`Failed to open folder: ${error}`, true);
  }
}

// Set status message
function setStatus(message: string, isError = false): void {
  if (!statusEl) return;

  statusEl.textContent = message;
  statusEl.classList.toggle("error", isError);
}

// HTML escape helper
function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
