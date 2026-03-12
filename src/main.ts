import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { WebviewWindow, getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
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

type CaptureMode = "window" | "region" | "display";
type ViewMode = CaptureMode;
type RecordingState = "idle" | "recording" | "saving";

interface AudioConfig {
  enabled: boolean;
  source_id: string | null;
  microphone_id: string | null;
  echo_cancellation: boolean;
  agc_enabled: boolean;
  agc_noise_gate_enabled: boolean;
}

interface TranscriptionConfig {
  enabled: boolean;
  model: string;
  show_transcript_window: boolean;
}

// Model status response
interface ModelStatus {
  model: string;
  display_name: string;
  path: string;
  exists: boolean;
  file_size: number | null;
  expected_size: number;
  size_display: string;
}

type ThemeMode = "auto" | "light" | "dark";

interface AppConfig {
  output: {
    directory: string | null;
  };
  audio: {
    enabled: boolean;
    source_id: string | null;
    microphone_id: string | null;
    echo_cancellation: boolean;
  };
  appearance: {
    theme: ThemeMode;
  };
}

interface RecordingResult {
  success: boolean;
  file_path: string | null;
  source_path: string | null;
  error: string | null;
}

interface ThumbnailResponse {
  data: string;
  width: number;
  height: number;
}

// Per-item thumbnail refresh timestamps
// Keys: "window:{handle}" or "display:{id}"
const lastRefreshed = new Map<string, number>();

// Minimum interval between captures for a single item (5 seconds)
const THUMBNAIL_MIN_INTERVAL_MS = 5000;
// Background refresh interval - captures visible stale items (20 seconds)
const THUMBNAIL_BACKGROUND_INTERVAL_MS = 20000;

function isStale(id: string, thresholdMs: number): boolean {
  return Date.now() - (lastRefreshed.get(id) ?? 0) >= thresholdMs;
}

function recordRefresh(id: string): void {
  lastRefreshed.set(id, Date.now());
}

// Viewport visibility tracking via IntersectionObserver
const visibleItems = new Set<string>();
const pendingCapture = new Set<string>();

// App-window visibility (Page Visibility API)
let appWindowVisible = !document.hidden;

// DOM Elements
let windowListEl: HTMLElement | null;
let windowSelectionEl: HTMLElement | null;
let regionSelectionEl: HTMLElement | null;
let regionDisplayEl: HTMLElement | null;
let displaySelectionEl: HTMLElement | null;
let displayListEl: HTMLElement | null;
let recordBtn: HTMLButtonElement | null;
let selectRegionBtn: HTMLButtonElement | null;
let modeWindowBtn: HTMLButtonElement | null;
let modeRegionBtn: HTMLButtonElement | null;
let modeDisplayBtn: HTMLButtonElement | null;
let timerEl: HTMLElement | null;
let statusOverlayEl: HTMLElement | null;
let statusDismissTimeout: number | null = null;
let resultDismissTimeout: number | null = null;
let resultEl: HTMLElement | null;
let resultPathEl: HTMLElement | null;
let openFolderBtn: HTMLButtonElement | null;
let appVersionEl: HTMLElement | null;
let permissionNoticeEl: HTMLElement | null;
let captureUiEl: HTMLElement | null;
let openSettingsBtn: HTMLButtonElement | null;
let closeBtn: HTMLButtonElement | null;

let modeConfigBtn: HTMLButtonElement | null;
let modeAboutBtn: HTMLButtonElement | null;
let transcriptionQuickToggle: HTMLElement | null;
let transcriptionQuickCheckbox: HTMLInputElement | null;

// Platform state
let isTrayModeDesktop = false;
type DesktopEnvironment = "gnome" | "kde" | "cosmic" | "cinnamon" | "hyprland" | "unknown";
let desktopEnvironment: DesktopEnvironment = "unknown";

// State
let captureMode: CaptureMode = "window";
let selectedWindow: WindowInfo | null = null;
let selectedRegion: CaptureRegion | null = null;
let selectedDisplay: MonitorInfo | null = null;
let regionSelectorWindow: WebviewWindow | null = null;
let currentState: RecordingState = "idle";
let timerInterval: number | null = null;
let recordingStartTime: number = 0;
// Theme state
let currentThemeMode: ThemeMode = "auto";
let systemThemeMediaQuery: MediaQueryList | null = null;
let themeChangeListenerRegistered = false;

// Thumbnail refresh state
let windowThumbnailRefreshInterval: number | null = null;
let displayThumbnailRefreshInterval: number | null = null;
let metadataPollInterval: number | null = null;
let regionPreviewData: string | null = null;
let lastRegionPreviewTime = 0;
let regionPreviewPendingTimeout: number | null = null;
const REGION_PREVIEW_THROTTLE_MS = 500; // 500ms throttle

// Stored region selector geometry (for persistence across close/reopen)
interface SelectorGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}
let storedSelectorGeometry: SelectorGeometry | null = null;

// Close region selector and store its geometry for later restoration
async function closeRegionSelector(): Promise<void> {
  if (!regionSelectorWindow) return;
  
  try {
    // Get current position and size before closing
    // Tauri returns physical pixels, convert to logical for WebviewWindow creation
    const scaleFactor = await regionSelectorWindow.scaleFactor();
    const pos = await regionSelectorWindow.outerPosition();
    const size = await regionSelectorWindow.innerSize();
    
    storedSelectorGeometry = {
      x: Math.round(pos.x / scaleFactor),
      y: Math.round(pos.y / scaleFactor),
      width: Math.round(size.width / scaleFactor),
      height: Math.round(size.height / scaleFactor),
    };
    console.log("Stored selector geometry (logical):", storedSelectorGeometry, "from physical, scale:", scaleFactor);
  } catch (e) {
    console.warn("Failed to get selector geometry:", e);
  }
  
  try {
    await regionSelectorWindow.close();
  } catch (e) {
    console.warn("Failed to close selector:", e);
  }
  
  regionSelectorWindow = null;
  updateRegionDisplay();
}

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  windowListEl = document.querySelector("#window-list");
  windowSelectionEl = document.querySelector("#window-selection");
  regionSelectionEl = document.querySelector("#region-selection");
  regionDisplayEl = document.querySelector("#region-display");
  displaySelectionEl = document.querySelector("#display-selection");
  displayListEl = document.querySelector("#display-list");
  recordBtn = document.querySelector("#record-btn");

  selectRegionBtn = document.querySelector("#select-region-btn");
  modeWindowBtn = document.querySelector("#mode-window-btn");
  modeRegionBtn = document.querySelector("#mode-region-btn");
  modeDisplayBtn = document.querySelector("#mode-display-btn");
  timerEl = document.querySelector("#timer");
  statusOverlayEl = document.querySelector("#status-overlay");
  resultEl = document.querySelector("#result");
  resultPathEl = document.querySelector("#result-path");
  openFolderBtn = document.querySelector("#open-folder-btn");
  appVersionEl = document.querySelector("#app-version");
  permissionNoticeEl = document.querySelector("#permission-notice");
  captureUiEl = document.querySelector("#capture-ui");
  openSettingsBtn = document.querySelector("#open-settings-btn");
  closeBtn = document.querySelector("#close-btn");
  modeConfigBtn = document.querySelector("#mode-config-btn");
  modeAboutBtn = document.querySelector("#mode-about-btn");
  transcriptionQuickToggle = document.querySelector("#transcription-quick-toggle");
  transcriptionQuickCheckbox = document.querySelector("#transcription-quick-checkbox");

  // Set up event listeners
  closeBtn?.addEventListener("click", async () => {
    // Close secondary windows first
    await closeSecondaryWindows();
    // Close region selector if open
    if (regionSelectorWindow) {
      try {
        await regionSelectorWindow.close();
      } catch (e) {
        console.warn("Failed to close selector:", e);
      }
      regionSelectorWindow = null;
    }
    getCurrentWebviewWindow().close();
  });
  
  // Also listen for window close event (e.g., from OS close button or Alt+F4)
  getCurrentWebviewWindow().onCloseRequested(async (_event) => {
    // Close secondary windows
    await closeSecondaryWindows();
    // If selector is open, close it first but don't prevent the main window close
    if (regionSelectorWindow) {
      try {
        await regionSelectorWindow.close();
      } catch (e) {
        console.warn("Failed to close selector on close request:", e);
      }
      regionSelectorWindow = null;
    }
    // Allow the close to proceed (don't call event.preventDefault())
  });
  statusOverlayEl?.addEventListener("click", dismissStatus);
  resultEl?.addEventListener("click", (e) => {
    // Don't dismiss if clicking the Open Folder button
    if (e.target !== openFolderBtn) {
      dismissResult();
    }
  });
  openSettingsBtn?.addEventListener("click", handleOpenSettings);
  recordBtn?.addEventListener("click", handleRecordClick);
  openFolderBtn?.addEventListener("click", handleOpenFolder);
  selectRegionBtn?.addEventListener("click", openRegionSelector);
  modeWindowBtn?.addEventListener("click", () => setViewMode("window"));
  modeRegionBtn?.addEventListener("click", () => setViewMode("region"));
  modeDisplayBtn?.addEventListener("click", () => setViewMode("display"));
  modeConfigBtn?.addEventListener("click", () => openConfigWindow());
  modeAboutBtn?.addEventListener("click", () => openAboutWindow());
  transcriptionQuickCheckbox?.addEventListener("change", handleTranscriptionQuickToggleChange);

  // Listen for region updates from selector window (continuous updates as user moves/resizes)
  listen<CaptureRegion>("region-updated", (event) => {
    console.log("Received region-updated:", event.payload);
    selectedRegion = event.payload;
    updateRegionDisplay();
    updateRecordButton();
  });

  // Listen for selector window closed (e.g., user pressed Escape)
  listen<SelectorGeometry | Record<string, never>>("region-selector-closed", (event) => {
    // Store geometry from event payload (sent by selector before closing)
    const payload = event.payload;
    console.log("region-selector-closed event received:", payload);
    if (payload && 'x' in payload && 'width' in payload) {
      storedSelectorGeometry = payload as SelectorGeometry;
      console.log("Stored selector geometry from close event:", storedSelectorGeometry);
    }
    regionSelectorWindow = null;
    // Update display to show details/button instead of fullsize preview
    // Keep the selected region - user can still record it
    updateRegionDisplay();
  });

  // Listen for tray menu events (GNOME mode)
  listen("tray-start-recording", () => {
    console.log("[Tray] Start recording event received");
    handleTrayStartRecording();
  });

  listen("tray-stop-recording", () => {
    console.log("[Tray] Stop recording event received");
    handleTrayStopRecording();
  });

  listen("tray-show-config", () => {
    console.log("[Tray] Show config event received");
    openConfigWindow();
  });

  listen("tray-show-about", () => {
    console.log("[Tray] Show about event received");
    openAboutWindow();
  });

  listen("tray-exit", () => {
    console.log("[Tray] Exit event received");
    // Stop any active recording before exit
    if (currentState === "recording") {
      stopRecording();
    }
  });

  listen("tray-show-transcription", () => {
    console.log("[Tray] Show transcription event received");
    handleTrayShowTranscription();
  });

  // Listen for external recording stop (e.g., user clicked GNOME's recording indicator)
  listen("recording-stream-stopped", () => {
    console.log("[Stream] Recording stream stopped externally");
    if (currentState === "recording") {
      handleTrayStopRecording();
    }
  });

  // Page Visibility API - pause/resume all refresh when app window hides/shows
  document.addEventListener("visibilitychange", () => {
    appWindowVisible = !document.hidden;
    if (!appWindowVisible) {
      stopMetadataPoll();
    } else {
      // Resume appropriate poll for the current tab
      if (captureMode === "window" || captureMode === "display") {
        startMetadataPoll();
      }
    }
  });

  // Initial load - detect desktop environment and check permissions
  detectDesktopEnvironment();
  checkPermissionsAndLoad();
  loadAppVersion();
  loadConfig();

});

// Check screen recording permission and show appropriate UI
async function checkPermissionsAndLoad(): Promise<void> {
  const permissionStatus = await invoke<string>("check_screen_recording_permission");
  
  if (permissionStatus === "denied") {
    showPermissionNotice();
  } else {
    hidePermissionNotice();
    loadWindows();
  }
}

// Show the permission notice and hide capture UI
function showPermissionNotice(): void {
  permissionNoticeEl?.classList.remove("hidden");
  captureUiEl?.classList.add("hidden");
  setStatus("Screen recording permission required", true);
}

// Hide the permission notice and show capture UI
function hidePermissionNotice(): void {
  permissionNoticeEl?.classList.add("hidden");
  captureUiEl?.classList.remove("hidden");
}

// Handle clicking the "Open System Settings" button
async function handleOpenSettings(): Promise<void> {
  // Open System Settings to the Screen Recording pane
  await invoke("open_screen_recording_settings");
}

// Load and display app version
async function loadAppVersion(): Promise<void> {
  try {
    const version = await getVersion();
    if (appVersionEl) {
      appVersionEl.textContent = `v${version}`;
    }
  } catch (error) {
    console.error("Failed to load app version:", error);
  }
}

// Detect desktop environment (GNOME, KDE, COSMIC, Cinnamon, Hyprland, etc.)
// Note: Cinnamon is detected but NOT treated as tray-mode because xdg-desktop-portal-xapp
// does not implement the ScreenCast interface required for screen recording.
async function detectDesktopEnvironment(): Promise<void> {
  try {
    desktopEnvironment = await invoke<DesktopEnvironment>("get_desktop_environment");
    isTrayModeDesktop = desktopEnvironment === "gnome" || desktopEnvironment === "kde" || desktopEnvironment === "cosmic";
    console.log("[Desktop] Detected environment:", desktopEnvironment, ", isTrayMode:", isTrayModeDesktop);
    
    // Apply tray mode UI changes for GNOME, KDE, and COSMIC
    if (isTrayModeDesktop) {
      applyTrayMode();
    }
  } catch (error) {
    console.error("Failed to detect desktop environment:", error);
  }
}

// Apply tray mode UI modifications (for GNOME, KDE, and COSMIC)
function applyTrayMode(): void {
  console.log("[TrayMode] Applying tray mode for", desktopEnvironment, "...");
  
  // Hide capture mode tabs (Window, Region, Display) - portal handles selection
  modeWindowBtn?.classList.add("hidden");
  modeRegionBtn?.classList.add("hidden");
  modeDisplayBtn?.classList.add("hidden");
  
  // Open config window in tray mode
  openConfigWindow();
}

// Set view mode (capture mode only)
function setViewMode(mode: ViewMode): void {
  if (currentState !== "idle") return;

  // Update button states for capture tabs
  modeWindowBtn?.classList.toggle("active", mode === "window");
  modeRegionBtn?.classList.toggle("active", mode === "region");
  modeDisplayBtn?.classList.toggle("active", mode === "display");

  // Show/hide sections
  windowSelectionEl?.classList.toggle("hidden", mode !== "window");
  regionSelectionEl?.classList.toggle("hidden", mode !== "region");
  displaySelectionEl?.classList.toggle("hidden", mode !== "display");

  captureMode = mode;

  // Clear window selection when switching away from window mode
  if (mode !== "window") {
    selectedWindow = null;
    document.querySelectorAll(".window-item.selected").forEach((el) => {
      el.classList.remove("selected");
    });
    stopWindowThumbnailRefresh();
  }

  // Clear region preview when switching away from region mode
  if (mode !== "region") {
    regionPreviewData = null;
  }

  // Stop display refresh when switching away from display mode
  if (mode !== "display") {
    stopDisplayThumbnailRefresh();
  }

  // Stop metadata poll - it will be restarted by the load/start functions below
  stopMetadataPoll();

  // Load displays when switching to display mode
  if (mode === "display") {
    loadDisplays();
  }

  // Restart window refresh when switching to window mode, and trigger immediate
  // evaluation of all currently-visible items (tab-activation scheduling rule)
  if (mode === "window") {
    startWindowThumbnailRefresh();
    startMetadataPoll();
    // Queue all visible stale items for immediate capture
    for (const id of visibleItems) {
      if (id.startsWith("window:") && isStale(id, THUMBNAIL_MIN_INTERVAL_MS)) {
        pendingCapture.add(id);
      }
    }
    if (pendingCapture.size > 0) {
      drainPendingCaptures().catch(err => console.error("Drain error:", err));
    }
  }

  updateRecordButton();
}

// Update region display
function updateRegionDisplay(): void {
  if (!regionDisplayEl) return;

  if (selectedRegion) {
    regionDisplayEl.classList.add("has-selection");
    
    // Check if selector is currently active
    const selectorActive = regionSelectorWindow !== null;
    
    // Build the preview image element if we have preview data
    const previewImg = regionPreviewData 
      ? `<img class="region-preview__img" src="data:image/jpeg;base64,${regionPreviewData}" alt="Region preview" />`
      : '<div class="region-preview__placeholder"></div>';
    
    if (selectorActive) {
      regionDisplayEl.classList.add("selector-active");
    } else {
      regionDisplayEl.classList.remove("selector-active");
    }
    
    // Always show fullsize preview with overlay info
    regionDisplayEl.innerHTML = `
      <div class="region-preview region-preview--fullsize">
        ${previewImg}
        <div class="region-overlay">
          <div class="region-overlay__info">
            <div class="region-overlay__dimensions">${selectedRegion.width} x ${selectedRegion.height}</div>
            <div class="region-overlay__monitor">${selectedRegion.monitor_name}</div>
          </div>
          ${!selectorActive ? '<button id="select-region-btn" type="button">Change Region</button>' : ''}
        </div>
      </div>
    `;
    
    // Re-attach event listener since we replaced the DOM
    if (!selectorActive) {
      document.querySelector("#select-region-btn")?.addEventListener("click", openRegionSelector);
    }
    
    // Load region preview (throttled)
    loadRegionPreviewThrottled();
  } else {
    regionDisplayEl.classList.remove("has-selection");
    regionDisplayEl.classList.remove("selector-active");
    regionPreviewData = null;
    regionDisplayEl.innerHTML = '<button id="select-region-btn" type="button">Select Region</button>';
    // Re-attach event listener since we replaced the DOM
    document.querySelector("#select-region-btn")?.addEventListener("click", openRegionSelector);
  }
}

// Load region preview (throttled with trailing edge)
function loadRegionPreviewThrottled(): void {
  const now = Date.now();
  const timeSinceLastUpdate = now - lastRegionPreviewTime;
  
  if (timeSinceLastUpdate >= REGION_PREVIEW_THROTTLE_MS) {
    // Enough time has passed, update immediately
    lastRegionPreviewTime = now;
    
    // Clear any pending update
    if (regionPreviewPendingTimeout !== null) {
      clearTimeout(regionPreviewPendingTimeout);
      regionPreviewPendingTimeout = null;
    }
    
    loadRegionPreview();
  } else {
    // Throttled - schedule an update for when the throttle period ends
    if (regionPreviewPendingTimeout === null) {
      const delay = REGION_PREVIEW_THROTTLE_MS - timeSinceLastUpdate;
      regionPreviewPendingTimeout = window.setTimeout(() => {
        regionPreviewPendingTimeout = null;
        lastRegionPreviewTime = Date.now();
        loadRegionPreview();
      }, delay);
    }
  }
}

// Load the region preview image
async function loadRegionPreview(): Promise<void> {
  if (!selectedRegion) return;
  
  try {
    const result = await invoke<ThumbnailResponse | null>("get_region_preview", {
      monitorId: selectedRegion.monitor_id,
      x: Math.round(selectedRegion.x),
      y: Math.round(selectedRegion.y),
      width: Math.round(selectedRegion.width),
      height: Math.round(selectedRegion.height),
    });
    
    if (result && result.data) {
      regionPreviewData = result.data;
      // Update the preview image if the region display still exists
      const img = document.querySelector<HTMLImageElement>(".region-preview__img");
      const placeholder = document.querySelector(".region-preview__placeholder");
      
      if (img) {
        img.src = `data:image/jpeg;base64,${result.data}`;
      } else if (placeholder && regionDisplayEl) {
        // Replace placeholder with image
        const previewContainer = document.querySelector(".region-preview");
        if (previewContainer) {
          previewContainer.innerHTML = `<img class="region-preview__img" src="data:image/jpeg;base64,${result.data}" alt="Region preview" />`;
        }
      }
    }
  } catch (error) {
    console.error("Failed to load region preview:", error);
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

    // Use stored geometry if available, otherwise default to centered on primary monitor
    let startX: number;
    let startY: number;
    let startWidth: number;
    let startHeight: number;
    
    if (storedSelectorGeometry) {
      // Restore previous position and size (already in logical coordinates)
      console.log("Restoring selector geometry (logical):", storedSelectorGeometry);
      startX = storedSelectorGeometry.x;
      startY = storedSelectorGeometry.y;
      startWidth = storedSelectorGeometry.width;
      startHeight = storedSelectorGeometry.height;
    } else {
      console.log("No stored geometry, using defaults");
      // Default selection size and position (centered on primary monitor)
      // Monitor coordinates are physical, convert to logical for WebviewWindow
      const scale = primaryMonitor.scale_factor;
      const monitorLogicalX = Math.round(primaryMonitor.x / scale);
      const monitorLogicalY = Math.round(primaryMonitor.y / scale);
      const monitorLogicalWidth = Math.round(primaryMonitor.width / scale);
      const monitorLogicalHeight = Math.round(primaryMonitor.height / scale);
      
      startWidth = 640;
      startHeight = 480;
      startX = monitorLogicalX + Math.floor((monitorLogicalWidth - startWidth) / 2);
      startY = monitorLogicalY + Math.floor((monitorLogicalHeight - startHeight) / 2);
      console.log("Default position (logical):", { x: startX, y: startY, width: startWidth, height: startHeight });
    }

    // Determine the URL based on environment
    const isDev = window.location.hostname === "localhost";
    const overlayUrl = isDev
      ? "http://localhost:1420/src/selection-overlay.html"
      : "src/selection-overlay.html";



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
      width: startWidth,
      height: startHeight,
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
    
    // Configure Hyprland window rules for floating overlay (Linux only)
    try {
      await invoke("configure_region_selector_window", { windowLabel: "region-selector" });
    } catch (e) {
      console.warn("Failed to configure region selector window rules:", e);
    }
    
    // If we have stored geometry, move the window to that position (Hyprland only)
    // Wayland doesn't respect window position hints, so we use Hyprland IPC
    if (storedSelectorGeometry) {
      try {
        await invoke("move_region_selector", {
          x: storedSelectorGeometry.x,
          y: storedSelectorGeometry.y,
          width: storedSelectorGeometry.width,
          height: storedSelectorGeometry.height,
        });
      } catch (e) {
        console.warn("Failed to move region selector:", e);
      }
    }
    
    await selector.setFocus();

    // Set initial region immediately so Record button is enabled
    selectedRegion = {
      monitor_id: primaryMonitor.id,
      monitor_name: primaryMonitor.name,
      x: startX - primaryMonitor.x,
      y: startY - primaryMonitor.y,
      width: startWidth,
      height: startHeight,
    };
    updateRegionDisplay();
    updateRecordButton();

    console.log("Selector ready");

  } catch (error) {
    console.error("Error opening region selector:", error);
    setStatus(`Error: ${error}`, true);
  }
}

// Load available windows (initial load only - subsequent updates via reconcileWindowList)
async function loadWindows(): Promise<void> {
  if (!windowListEl) return;

  windowListEl.innerHTML = '<p class="loading">Loading windows...</p>';
  selectedWindow = null;
  updateRecordButton();

  // Stop any existing refresh intervals
  stopWindowThumbnailRefresh();
  stopMetadataPoll();

  try {
    const windows = await invoke<WindowInfo[]>("get_windows");

    if (windows.length === 0) {
      // Re-check permission in case it was revoked
      const permissionStatus = await invoke<string>("check_screen_recording_permission");
      if (permissionStatus === "denied") {
        showPermissionNotice();
        return;
      }
      
      windowListEl.innerHTML = '<p class="empty">No capturable windows found</p>';
      return;
    }

    windowListEl.innerHTML = "";
    for (const win of windows) {
      const item = createWindowItem(win);
      windowListEl.appendChild(item);
    }

    // Thumbnails are loaded by IntersectionObserver as items enter the viewport.
    // Start auto-refresh and metadata polling.
    startWindowThumbnailRefresh();
    startMetadataPoll();
  } catch (error) {
    windowListEl.innerHTML = `<p class="error">Error loading windows: ${error}</p>`;
    setStatus(`Error: ${error}`, true);
  }
}

// =============================================================================
// IntersectionObserver - viewport visibility tracking
// =============================================================================

const listIntersectionObserver = new IntersectionObserver((entries) => {
  let added = false;
  for (const entry of entries) {
    const el = entry.target as HTMLElement;
    const id = el.dataset.itemId;
    if (!id) continue;

    if (entry.isIntersecting) {
      visibleItems.add(id);
      // Schedule immediate capture if stale (scroll-into-view rule: 5s threshold)
      if (isStale(id, THUMBNAIL_MIN_INTERVAL_MS)) {
        pendingCapture.add(id);
        added = true;
      }
    } else {
      visibleItems.delete(id);
    }
  }
  // Drain immediately so thumbnails load on first show without requiring hover
  if (added) {
    drainPendingCaptures().catch(err => console.error("Drain error:", err));
  }
}, { threshold: 0.1 });

// =============================================================================
// Metadata poll loop - lightweight list reconciliation (no thumbnail capture)
// =============================================================================

function startMetadataPoll(): void {
  if (metadataPollInterval !== null) return;
  metadataPollInterval = window.setInterval(() => {
    if (!appWindowVisible || currentState !== "idle") return;
    if (captureMode === "window") {
      reconcileWindowList().catch(err => console.error("Window reconcile error:", err));
    } else if (captureMode === "display") {
      reconcileDisplayList().catch(err => console.error("Display reconcile error:", err));
    }
  }, 5000);
}

function stopMetadataPoll(): void {
  if (metadataPollInterval !== null) {
    clearInterval(metadataPollInterval);
    metadataPollInterval = null;
  }
}

// Incrementally reconcile the window list DOM without clearing thumbnails or scroll position
async function reconcileWindowList(): Promise<void> {
  if (!windowListEl) return;
  try {
    const windows = await invoke<WindowInfo[]>("get_windows");
    const newHandles = new Set(windows.map(w => w.handle));

    // Build map of current DOM items keyed by handle
    const existingItems = windowListEl.querySelectorAll<HTMLElement>(".window-item");
    const existingHandles = new Set<number>();

    existingItems.forEach((item) => {
      const handle = parseInt(item.dataset.handle || "0", 10);
      if (!newHandles.has(handle)) {
        // Window closed - remove item and clean up tracking
        const id = item.dataset.itemId;
        if (id) {
          listIntersectionObserver.unobserve(item);
          visibleItems.delete(id);
          lastRefreshed.delete(id);
          pendingCapture.delete(id);
        }
        item.remove();
        if (selectedWindow?.handle === handle) {
          selectedWindow = null;
          updateRecordButton();
        }
      } else {
        existingHandles.add(handle);
        // Update metadata in place (title, process name)
        const win = windows.find(w => w.handle === handle);
        if (win) {
          const titleEl = item.querySelector(".window-item__title");
          const processEl = item.querySelector(".window-item__process");
          if (titleEl) titleEl.textContent = win.title;
          if (processEl) processEl.textContent = win.process_name;
        }
      }
    });

    // Handle empty state
    if (windows.length === 0) {
      if (!windowListEl.querySelector(".empty")) {
        windowListEl.innerHTML = '<p class="empty">No capturable windows found</p>';
      }
      return;
    }

    const emptyMsg = windowListEl.querySelector(".empty");
    if (emptyMsg) emptyMsg.remove();

    // Append new windows
    for (const win of windows) {
      if (!existingHandles.has(win.handle)) {
        const item = createWindowItem(win);
        windowListEl.appendChild(item);
      }
    }
  } catch (error) {
    console.error("Error reconciling window list:", error);
  }
}

// Incrementally reconcile the display list DOM without clearing thumbnails or scroll position
async function reconcileDisplayList(): Promise<void> {
  if (!displayListEl) return;
  try {
    const displays = await invoke<MonitorInfo[]>("get_monitors");
    const newIds = new Set(displays.map(d => d.id));

    const existingItems = displayListEl.querySelectorAll<HTMLElement>(".display-item");
    const existingIds = new Set<string>();

    existingItems.forEach((item) => {
      const id = item.dataset.id;
      if (!id || !newIds.has(id)) {
        const itemId = item.dataset.itemId;
        if (itemId) {
          listIntersectionObserver.unobserve(item);
          visibleItems.delete(itemId);
          lastRefreshed.delete(itemId);
          pendingCapture.delete(itemId);
        }
        item.remove();
        if (selectedDisplay?.id === id) {
          selectedDisplay = null;
          updateRecordButton();
        }
      } else {
        existingIds.add(id);
        // Update metadata in place
        const display = displays.find(d => d.id === id);
        if (display) {
          const nameEl = item.querySelector(".display-item__name");
          const resEl = item.querySelector(".display-item__resolution");
          if (nameEl) {
            const primaryBadge = display.is_primary
              ? '<span class="display-item__primary">Primary</span>'
              : "";
            nameEl.innerHTML = escapeHtml(display.name) + primaryBadge;
          }
          if (resEl) resEl.textContent = `${display.width} x ${display.height}`;
        }
      }
    });

    if (displays.length === 0) {
      if (!displayListEl.querySelector(".empty")) {
        displayListEl.innerHTML = '<p class="empty">No displays found</p>';
      }
      return;
    }

    const emptyMsg = displayListEl.querySelector(".empty");
    if (emptyMsg) emptyMsg.remove();

    for (const display of displays) {
      if (!existingIds.has(display.id)) {
        const item = createDisplayItem(display);
        displayListEl.appendChild(item);
      }
    }
  } catch (error) {
    console.error("Error reconciling display list:", error);
  }
}

// =============================================================================
// Pending capture drain - processes queued IDs in small batches
// =============================================================================

let drainInProgress = false;

async function drainPendingCaptures(): Promise<void> {
  if (drainInProgress) return;
  if (!appWindowVisible || currentState !== "idle") return;

  drainInProgress = true;
  try {
    // Snapshot the pending set and clear it before processing
    const ids = [...pendingCapture];
    pendingCapture.clear();

    // Process in batches of 2 to avoid burst load
    const BATCH_SIZE = 2;
    for (let i = 0; i < ids.length; i += BATCH_SIZE) {
      if (!appWindowVisible || currentState !== "idle") break;

      const batch = ids.slice(i, i + BATCH_SIZE);
      await Promise.all(batch.map(id => captureItemById(id)));
    }
  } finally {
    drainInProgress = false;
    // Process any items that were added while draining
    if (pendingCapture.size > 0 && appWindowVisible && currentState === "idle") {
      drainPendingCaptures().catch(err => console.error("Drain error:", err));
    }
  }
}

// Capture a single item by its tracking ID, enforcing the hard minimum interval
async function captureItemById(id: string): Promise<void> {
  // Hard minimum: never capture more than once per 5 seconds
  if (!isStale(id, THUMBNAIL_MIN_INTERVAL_MS)) return;
  // Item must still be visible in the viewport
  if (!visibleItems.has(id)) return;

  if (id.startsWith("window:")) {
    const handle = parseInt(id.slice(7), 10);
    const item = windowListEl?.querySelector<HTMLElement>(`[data-handle="${handle}"]`);
    const img = item?.querySelector<HTMLImageElement>(".window-item__thumb-img");
    if (!img) return;
    await captureWindowThumbnailDirect(handle, img);
  } else if (id.startsWith("display:")) {
    const monitorId = id.slice(8);
    const item = displayListEl?.querySelector<HTMLElement>(`[data-item-id="${CSS.escape(id)}"]`);
    const img = item?.querySelector<HTMLImageElement>(".display-item__thumb-img");
    if (!img) return;
    await captureDisplayThumbnailDirect(monitorId, img);
  }
}

// =============================================================================
// Window thumbnail refresh scheduler
// =============================================================================

// Start auto-refresh for window thumbnails
function startWindowThumbnailRefresh(): void {
  if (windowThumbnailRefreshInterval !== null) return;
  
  windowThumbnailRefreshInterval = window.setInterval(() => {
    if (!appWindowVisible || currentState !== "idle" || captureMode !== "window") return;
    tickWindowThumbnails();
    drainPendingCaptures().catch(err => console.error("Drain error:", err));
  }, THUMBNAIL_BACKGROUND_INTERVAL_MS);
}

// Stop auto-refresh for window thumbnails
function stopWindowThumbnailRefresh(): void {
  if (windowThumbnailRefreshInterval !== null) {
    clearInterval(windowThumbnailRefreshInterval);
    windowThumbnailRefreshInterval = null;
  }
}

// Background tick: queue visible items stale beyond 20s (respecting 5s hard min)
function tickWindowThumbnails(): void {
  for (const id of visibleItems) {
    if (id.startsWith("window:") && isStale(id, THUMBNAIL_BACKGROUND_INTERVAL_MS)) {
      pendingCapture.add(id);
    }
  }
}

// Create a window list item element
function createWindowItem(win: WindowInfo): HTMLElement {
  const item = document.createElement("div");
  item.className = "window-item";
  item.dataset.handle = String(win.handle);
  const itemId = `window:${win.handle}`;
  item.dataset.itemId = itemId;

  item.innerHTML = `
    <div class="window-item__thumbnail">
      <img class="window-item__thumb-img" alt="" />
      <div class="window-item__thumb-placeholder"></div>
    </div>
    <div class="window-item__info">
      <div class="window-item__title">${escapeHtml(win.title)}</div>
      <div class="window-item__process">${escapeHtml(win.process_name)}</div>
    </div>
  `;

  item.addEventListener("click", () => selectWindow(win, item));

  // Hover-triggered refresh: queue capture if item is stale (5s threshold)
  item.addEventListener("mouseenter", () => {
    if (!appWindowVisible || currentState !== "idle" || captureMode !== "window") return;
    if (isStale(itemId, THUMBNAIL_MIN_INTERVAL_MS)) {
      pendingCapture.add(itemId);
      drainPendingCaptures().catch(err => console.error("Drain error:", err));
    }
  });

  // Observe for viewport visibility (IntersectionObserver handles thumbnail scheduling)
  listIntersectionObserver.observe(item);

  return item;
}

// Direct window thumbnail capture (invokes Tauri command)
async function captureWindowThumbnailDirect(handle: number, imgElement: HTMLImageElement): Promise<void> {
  const id = `window:${handle}`;
  // Final hard-minimum check before the IPC call
  if (!isStale(id, THUMBNAIL_MIN_INTERVAL_MS)) return;

  const result = await invoke<ThumbnailResponse | null>("get_window_thumbnail", {
    windowHandle: handle,
  });

  if (result && result.data) {
    recordRefresh(id);
    if (document.contains(imgElement)) {
      imgElement.src = `data:image/jpeg;base64,${result.data}`;
      imgElement.classList.add("loaded");
    }
  }
}

// Load available displays (initial load only - subsequent updates via reconcileDisplayList)
async function loadDisplays(): Promise<void> {
  if (!displayListEl) return;

  displayListEl.innerHTML = '<p class="loading">Loading displays...</p>';
  selectedDisplay = null;
  updateRecordButton();

  // Stop any existing refresh intervals
  stopDisplayThumbnailRefresh();
  stopMetadataPoll();

  try {
    const displays = await invoke<MonitorInfo[]>("get_monitors");

    if (displays.length === 0) {
      displayListEl.innerHTML = '<p class="empty">No displays found</p>';
      return;
    }

    displayListEl.innerHTML = "";
    for (const display of displays) {
      const item = createDisplayItem(display);
      displayListEl.appendChild(item);
    }

    // Thumbnails are loaded by IntersectionObserver as items enter the viewport.
    // Start auto-refresh and metadata polling.
    startDisplayThumbnailRefresh();
    startMetadataPoll();
  } catch (error) {
    displayListEl.innerHTML = `<p class="error">Error loading displays: ${error}</p>`;
    setStatus(`Error: ${error}`, true);
  }
}

// Start auto-refresh for display thumbnails
function startDisplayThumbnailRefresh(): void {
  if (displayThumbnailRefreshInterval !== null) return;
  
  displayThumbnailRefreshInterval = window.setInterval(() => {
    if (!appWindowVisible || currentState !== "idle" || captureMode !== "display") return;
    tickDisplayThumbnails();
    drainPendingCaptures().catch(err => console.error("Drain error:", err));
  }, THUMBNAIL_BACKGROUND_INTERVAL_MS);
}

// Stop auto-refresh for display thumbnails
function stopDisplayThumbnailRefresh(): void {
  if (displayThumbnailRefreshInterval !== null) {
    clearInterval(displayThumbnailRefreshInterval);
    displayThumbnailRefreshInterval = null;
  }
}

// Background tick: queue visible display items stale beyond 20s (respecting 5s hard min)
function tickDisplayThumbnails(): void {
  for (const id of visibleItems) {
    if (id.startsWith("display:") && isStale(id, THUMBNAIL_BACKGROUND_INTERVAL_MS)) {
      pendingCapture.add(id);
    }
  }
}

// Create a display list item element
function createDisplayItem(display: MonitorInfo): HTMLElement {
  const item = document.createElement("div");
  item.className = "display-item";
  item.dataset.id = display.id;
  const itemId = `display:${display.id}`;
  item.dataset.itemId = itemId;

  const primaryBadge = display.is_primary
    ? '<span class="display-item__primary">Primary</span>'
    : "";

  item.innerHTML = `
    <div class="display-item__thumbnail">
      <img class="display-item__thumb-img" alt="" />
      <div class="display-item__thumb-placeholder"></div>
    </div>
    <div class="display-item__info">
      <div class="display-item__name">${escapeHtml(display.name)}${primaryBadge}</div>
      <div class="display-item__resolution">${display.width} x ${display.height}</div>
    </div>
  `;

  item.addEventListener("click", () => selectDisplayItem(display, item));

  // Hover-triggered refresh: queue capture if item is stale (5s threshold)
  item.addEventListener("mouseenter", () => {
    if (!appWindowVisible || currentState !== "idle" || captureMode !== "display") return;
    if (isStale(itemId, THUMBNAIL_MIN_INTERVAL_MS)) {
      pendingCapture.add(itemId);
      drainPendingCaptures().catch(err => console.error("Drain error:", err));
    }
  });

  // Observe for viewport visibility (IntersectionObserver handles thumbnail scheduling)
  listIntersectionObserver.observe(item);

  return item;
}

// Direct display thumbnail capture (invokes Tauri command)
async function captureDisplayThumbnailDirect(monitorId: string, imgElement: HTMLImageElement): Promise<void> {
  const id = `display:${monitorId}`;
  // Final hard-minimum check before the IPC call
  if (!isStale(id, THUMBNAIL_MIN_INTERVAL_MS)) return;

  const result = await invoke<ThumbnailResponse | null>("get_display_thumbnail", {
    monitorId,
  });

  if (result && result.data) {
    recordRefresh(id);
    if (document.contains(imgElement)) {
      imgElement.src = `data:image/jpeg;base64,${result.data}`;
      imgElement.classList.add("loaded");
    }
  }
}

// Select a display for recording
function selectDisplayItem(display: MonitorInfo, element: HTMLElement): void {
  if (currentState !== "idle") return;

  // Remove selection from previous
  document.querySelectorAll(".display-item.selected").forEach((el) => {
    el.classList.remove("selected");
  });

  // Select new
  element.classList.add("selected");
  selectedDisplay = display;
  updateRecordButton();

  // Show highlight overlay on the selected display
  showDisplayHighlight(display);
}

// Show a brief highlight border on a display to help identify it
async function showDisplayHighlight(display: MonitorInfo): Promise<void> {
  try {
    await invoke("show_display_highlight", {
      monitorId: display.id,
    });
  } catch (error) {
    console.error("Error showing display highlight:", error);
  }
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

  // Show highlight overlay on the selected window
  showWindowHighlight(win);
}

// Show a brief highlight border on a window to help identify it
async function showWindowHighlight(win: WindowInfo): Promise<void> {
  try {
    await invoke("show_window_highlight", {
      windowHandle: win.handle,
    });
  } catch (error) {
    console.error("Error showing window highlight:", error);
  }
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

  if (captureMode === "display" && !selectedDisplay) {
    setStatus("Please select a display first", true);
    return;
  }
  
  // Check if transcription is enabled but model is not downloaded
  const transcriptionEnabled = transcriptionQuickCheckbox?.checked ?? false;
  if (transcriptionEnabled) {
    try {
      const txConfig = await invoke<TranscriptionConfig>("get_transcription_config");
      if (txConfig.model) {
        const status = await invoke<ModelStatus>("get_model_status", { model: txConfig.model });
        if (!status.exists) {
          setStatus(`Transcription model "${status.display_name}" not downloaded. Please download it first or disable transcription.`, true);
          return;
        }
      }
    } catch (error) {
      console.error("[Recording] Failed to check model status:", error);
      // Allow recording to proceed if we can't check status
    }
  }

  setStatus("Starting recording...");
  disableSelection(true);

  try {
    if (captureMode === "window" && selectedWindow) {
      await invoke("start_recording", { windowHandle: selectedWindow.handle });
    } else if (captureMode === "region" && selectedRegion) {
      console.log("Starting region recording with:", selectedRegion);
      
      // Hide the selector UI elements before recording starts
      if (regionSelectorWindow) {
        await regionSelectorWindow.emit("recording-started");
      }
      
      await invoke("start_region_recording", {
        monitorId: selectedRegion.monitor_id,
        x: Math.round(selectedRegion.x),
        y: Math.round(selectedRegion.y),
        width: Math.round(selectedRegion.width),
        height: Math.round(selectedRegion.height),
      });
    } else if (captureMode === "display" && selectedDisplay) {
      console.log("Starting display recording with:", selectedDisplay);
      await invoke("start_display_recording", {
        monitorId: selectedDisplay.id,
      });
    }

    currentState = "recording";
    updateRecordButton();
    startTimer();
    setStatus("Recording...");
    
    // Open transcript window if transcription is enabled and setting is on
    // Fire and forget - don't await to avoid blocking
    if (transcriptionEnabled) {
      // Check the show_transcript_window config setting
      let showTranscript = true; // default
      try {
        const txCfg = await invoke<TranscriptionConfig>("get_transcription_config");
        showTranscript = txCfg.show_transcript_window;
      } catch { /* use default */ }
      if (showTranscript) {
        invoke("open_transcript_window")
          .then(() => console.log("[Recording] Opened transcript window"))
          .catch((error) => console.error("[Recording] Failed to open transcript window:", error));
      }
    }
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
      
      // Close region selector after successful recording (preserves region state)
      if (captureMode === "region" && regionSelectorWindow) {
        await closeRegionSelector();
      }
    } else {
      setStatus(`Recording failed: ${result.error || "Unknown error"}`, true);
      // Show the selector UI elements again on failure
      if (regionSelectorWindow) {
        await regionSelectorWindow.emit("recording-stopped");
      }
    }
  } catch (error) {
    setStatus(`Error stopping recording: ${error}`, true);
    // Show the selector UI elements again on error
    if (regionSelectorWindow) {
      await regionSelectorWindow.emit("recording-stopped");
    }
  }

  currentState = "idle";
  updateRecordButton();
  disableSelection(false);
}

// =============================================================================
// GNOME Tray Recording Functions
// =============================================================================

// Handle tray "Start Recording" - invokes portal on GNOME
async function handleTrayStartRecording(): Promise<void> {
  console.log("[Tray] handleTrayStartRecording called, currentState:", currentState);
  if (currentState !== "idle") {
    console.log("[Tray] Cannot start recording - not in idle state");
    return;
  }

  console.log("[Tray] Starting portal-based recording...");
  setStatus("Starting recording...");

  try {
    // In tray mode (GNOME/KDE), we use portal-based recording which shows the native picker
    // The portal handles source selection, so we call a generic start function
    console.log("[Tray] Calling start_gnome_recording...");
    await invoke("start_gnome_recording");
    console.log("[Tray] Portal recording started");
    
    currentState = "recording";
    updateRecordButton();
    startTimer();
    setStatus("Recording...");
    
    // Update tray icon to recording state
    console.log("[Tray] Calling set_tray_recording_state(true)...");
    await invoke("set_tray_recording_state", { recording: true });
    console.log("[Tray] set_tray_recording_state completed");
  } catch (error) {
    console.error("[Tray] Failed to start recording:", error);
    setStatus(`Failed to start recording: ${error}`, true);
  }
}

// Handle tray "Stop Recording"
async function handleTrayStopRecording(): Promise<void> {
  if (currentState !== "recording") {
    console.log("[Tray] Cannot stop recording - not recording");
    return;
  }

  console.log("[Tray] Stopping recording...");
  await stopRecording();
  
  // Update tray icon back to normal state
  try {
    await invoke("set_tray_recording_state", { recording: false });
  } catch (error) {
    console.error("[Tray] Failed to update tray icon:", error);
  }
  
  // Close region selector if open (GNOME mode)
  if (regionSelectorWindow) {
    try {
      await regionSelectorWindow.close();
    } catch {
      // Window may already be closed
    }
    regionSelectorWindow = null;
  }
}

// Handle tray "Transcription" - show transcription window if active
async function handleTrayShowTranscription(): Promise<void> {
  // Check if transcription is currently active (recording with transcription enabled)
  const isTranscriptionActive = currentState === "recording" && (transcriptionQuickCheckbox?.checked ?? false);
  
  if (isTranscriptionActive) {
    // Open/show the transcription window
    try {
      await invoke("open_transcript_window");
      console.log("[Tray] Opened transcription window");
    } catch (error) {
      console.error("[Tray] Failed to open transcription window:", error);
      setStatus("Failed to open transcription window", true);
    }
  } else {
    // Transcription is not active - show error message
    console.log("[Tray] Transcription not active");
    setStatus("Transcription is not active. Start a recording with transcription enabled.", true);
  }
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
      } else if (captureMode === "region") {
        recordBtn.disabled = !selectedRegion;
      } else if (captureMode === "display") {
        recordBtn.disabled = !selectedDisplay;
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
  const currentSelectRegionBtn = document.querySelector<HTMLButtonElement>("#select-region-btn");
  if (currentSelectRegionBtn) {
    currentSelectRegionBtn.disabled = disabled;
  }
  if (modeWindowBtn) {
    modeWindowBtn.disabled = disabled;
  }
  if (modeRegionBtn) {
    modeRegionBtn.disabled = disabled;
  }
  if (modeDisplayBtn) {
    modeDisplayBtn.disabled = disabled;
  }
  if (modeConfigBtn) {
    modeConfigBtn.disabled = disabled;
  }
  if (modeAboutBtn) {
    modeAboutBtn.disabled = disabled;
  }

  document.querySelectorAll(".window-item").forEach((el) => {
    if (disabled) {
      el.classList.add("disabled");
    } else {
      el.classList.remove("disabled");
    }
  });

  document.querySelectorAll(".display-item").forEach((el) => {
    if (disabled) {
      el.classList.add("disabled");
    } else {
      el.classList.remove("disabled");
    }
  });

  // Disable transcription quick toggle during recording
  if (transcriptionQuickToggle) {
    transcriptionQuickToggle.classList.toggle("disabled", disabled);
  }
  if (transcriptionQuickCheckbox) {
    transcriptionQuickCheckbox.disabled = disabled;
  }
}

// Show result overlay with auto-dismiss
function showResult(filePath: string): void {
  if (!resultEl || !resultPathEl) return;

  // Clear any existing dismiss timeout
  if (resultDismissTimeout !== null) {
    clearTimeout(resultDismissTimeout);
    resultDismissTimeout = null;
  }

  resultPathEl.textContent = filePath;
  resultEl.classList.remove("hidden", "fade-out");

  // Store path for open folder button
  resultEl.dataset.path = filePath;

  // Auto-dismiss after 5 seconds
  resultDismissTimeout = window.setTimeout(() => {
    dismissResult();
  }, 5000);
}

// Dismiss the result overlay
function dismissResult(): void {
  if (!resultEl) return;

  // Clear timeout if manually dismissed
  if (resultDismissTimeout !== null) {
    clearTimeout(resultDismissTimeout);
    resultDismissTimeout = null;
  }

  // Fade out then hide
  resultEl.classList.add("fade-out");
  setTimeout(() => {
    resultEl?.classList.add("hidden");
    resultEl?.classList.remove("fade-out");
  }, 200);
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

// Set status message - shows as overlay that auto-dismisses
function setStatus(message: string, isError = false): void {
  // Log the message to console
  if (isError) {
    console.error("[Status]", message);
  } else {
    console.log("[Status]", message);
  }

  if (!statusOverlayEl) return;

  // Clear any existing dismiss timeout
  if (statusDismissTimeout !== null) {
    clearTimeout(statusDismissTimeout);
    statusDismissTimeout = null;
  }

  // Update content and show
  statusOverlayEl.textContent = message;
  statusOverlayEl.classList.toggle("error", isError);
  statusOverlayEl.classList.remove("hidden", "fade-out");

  // Auto-dismiss after 5 seconds
  statusDismissTimeout = window.setTimeout(() => {
    dismissStatus();
  }, 5000);
}

// Dismiss the status overlay
function dismissStatus(): void {
  if (!statusOverlayEl) return;

  // Clear timeout if manually dismissed
  if (statusDismissTimeout !== null) {
    clearTimeout(statusDismissTimeout);
    statusDismissTimeout = null;
  }

  // Fade out then hide
  statusOverlayEl.classList.add("fade-out");
  setTimeout(() => {
    statusOverlayEl?.classList.add("hidden");
    statusOverlayEl?.classList.remove("fade-out");
  }, 200);
}

// =============================================================================
// Configuration Functions (simplified - config UI is in separate window)
// =============================================================================

// Load configuration (theme and transcription quick toggle state only)
async function loadConfig(): Promise<void> {
  try {
    const config = await invoke<AppConfig>("get_config");

    // Initialize theme with saved preference
    const themeMode = config.appearance?.theme || "auto";
    initTheme(themeMode);

    // Load transcription state for quick toggle
    await loadTranscriptionQuickToggle();

    console.log("[Config] Loaded config, theme:", themeMode);
  } catch (error) {
    console.error("[Config] Failed to load config:", error);
  }
}

// Load transcription enabled state for the quick toggle
async function loadTranscriptionQuickToggle(): Promise<void> {
  try {
    const config = await invoke<TranscriptionConfig>("get_transcription_config");
    if (transcriptionQuickCheckbox) {
      transcriptionQuickCheckbox.checked = config.enabled;
    }

    // Update quick toggle visibility based on audio config
    const audioConfig = await invoke<AudioConfig>("get_audio_config");
    const platform = await invoke<string>("get_platform");
    let hasSystemAudio = false;
    if (platform === "macos") {
      hasSystemAudio = audioConfig.source_id === "system";
    } else {
      hasSystemAudio = audioConfig.source_id !== null && audioConfig.source_id !== "";
    }
    if (transcriptionQuickToggle) {
      transcriptionQuickToggle.classList.toggle("hidden", !hasSystemAudio);
    }
  } catch (error) {
    console.error("[Config] Failed to load transcription state:", error);
  }
}

// Handle quick toggle change (save to config)
async function handleTranscriptionQuickToggleChange(): Promise<void> {
  if (!transcriptionQuickCheckbox) return;

  const enabled = transcriptionQuickCheckbox.checked;

  try {
    await invoke("save_transcription_config", { enabled });
    console.log("[Transcription] Quick toggle: enabled=", enabled);
  } catch (error) {
    console.error("[Transcription] Failed to save config:", error);
  }
}

// =============================================================================
// Window Opening Functions
// =============================================================================

// Open the configuration window (single instance)
async function openConfigWindow(): Promise<void> {
  // Check if config window already exists
  const existing = await WebviewWindow.getByLabel("config");
  if (existing) {
    try {
      await existing.show();
      await existing.setFocus();
      return;
    } catch {
      // Window may have been destroyed, create a new one
    }
  }

  const isDev = window.location.hostname === "localhost";
  const url = isDev
    ? "http://localhost:1420/src/config.html"
    : "src/config.html";

  const configWindow = new WebviewWindow("config", {
    url,
    title: "OmniRec Settings",
    decorations: false,
    transparent: false,
    shadow: true,
    resizable: false,
    maximizable: false,
    width: 450,
    height: 550,
  });

  await new Promise<void>((resolve, reject) => {
    configWindow.once("tauri://created", () => {
      console.log("[Window] Config window created");
      resolve();
    });
    configWindow.once("tauri://error", (e) => {
      console.error("[Window] Failed to create config window:", e);
      reject(new Error(`Failed to create config window: ${e}`));
    });
  });
}

// Open the about window (single instance)
async function openAboutWindow(): Promise<void> {
  // Check if about window already exists
  const existing = await WebviewWindow.getByLabel("about");
  if (existing) {
    try {
      await existing.show();
      await existing.setFocus();
      return;
    } catch {
      // Window may have been destroyed, create a new one
    }
  }

  const isDev = window.location.hostname === "localhost";
  const url = isDev
    ? "http://localhost:1420/src/about.html"
    : "src/about.html";

  const aboutWindow = new WebviewWindow("about", {
    url,
    title: "About OmniRec",
    decorations: false,
    transparent: false,
    shadow: true,
    resizable: false,
    maximizable: false,
    width: 350,
    height: 400,
  });

  await new Promise<void>((resolve, reject) => {
    aboutWindow.once("tauri://created", () => {
      console.log("[Window] About window created");
      resolve();
    });
    aboutWindow.once("tauri://error", (e) => {
      console.error("[Window] Failed to create about window:", e);
      reject(new Error(`Failed to create about window: ${e}`));
    });
  });
}

// Close config and about windows (called when main window hides)
async function closeSecondaryWindows(): Promise<void> {
  const configWindow = await WebviewWindow.getByLabel("config");
  if (configWindow) {
    try { await configWindow.close(); } catch { /* ignore */ }
  }
  const aboutWindow = await WebviewWindow.getByLabel("about");
  if (aboutWindow) {
    try { await aboutWindow.close(); } catch { /* ignore */ }
  }
}

// =============================================================================
// Theme Functions
// =============================================================================

// Get the system's preferred color scheme
function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

// Determine the effective theme based on mode and system preference
function getEffectiveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "auto") {
    return getSystemTheme();
  }
  return mode;
}

// Apply the theme to the document
function applyTheme(theme: "light" | "dark"): void {
  const body = document.body;
  body.classList.remove("theme-light", "theme-dark");
  body.classList.add(`theme-${theme}`);
  console.log("[Theme] Applied theme:", theme);
}

// Initialize theme system
function initTheme(mode: ThemeMode): void {
  currentThemeMode = mode;
  const effectiveTheme = getEffectiveTheme(mode);
  applyTheme(effectiveTheme);

  systemThemeMediaQuery = window.matchMedia("(prefers-color-scheme: light)");
  systemThemeMediaQuery.addEventListener("change", handleSystemThemeChange);

  // Listen for theme changes from other windows (e.g. config window).
  // Re-register each time initTheme is called would create duplicate listeners,
  // so we guard with a module-level flag.
  if (!themeChangeListenerRegistered) {
    themeChangeListenerRegistered = true;
    listen<string>("theme-changed", (event) => {
      const newMode = event.payload as ThemeMode;
      currentThemeMode = newMode;
      applyTheme(getEffectiveTheme(newMode));
      console.log("[Theme] theme-changed event received, applied:", newMode);
    });
  }
}

// Handle system theme change
function handleSystemThemeChange(): void {
  if (currentThemeMode === "auto") {
    const effectiveTheme = getEffectiveTheme("auto");
    applyTheme(effectiveTheme);
    console.log("[Theme] System theme changed, applied:", effectiveTheme);
  }
}

// HTML escape helper
function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
