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
type ViewMode = CaptureMode | "config";
type RecordingState = "idle" | "recording" | "saving";
type OutputFormat = "mp4" | "webm" | "mkv" | "quicktime" | "gif" | "apng" | "webp";

interface AudioSource {
  id: string;
  name: string;
  source_type: "input" | "output";
}

interface AudioConfig {
  enabled: boolean;
  source_id: string | null;
  microphone_id: string | null;
  echo_cancellation: boolean;
}

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
}

interface RecordingResult {
  success: boolean;
  file_path: string | null;
  source_path: string | null;
  error: string | null;
}

interface TranscodingCompleteEvent {
  success: boolean;
  output_path?: string;
  source_path?: string;
  error?: string;
}

interface ThumbnailResponse {
  data: string;
  width: number;
  height: number;
}

// Thumbnail cache with TTL
const THUMBNAIL_CACHE_TTL_MS = 5000; // 5 seconds
const thumbnailCache = new Map<string, { data: string; timestamp: number }>();

function getCachedThumbnail(key: string): string | null {
  const cached = thumbnailCache.get(key);
  if (cached && Date.now() - cached.timestamp < THUMBNAIL_CACHE_TTL_MS) {
    return cached.data;
  }
  return null;
}

function setCachedThumbnail(key: string, data: string): void {
  thumbnailCache.set(key, { data, timestamp: Date.now() });
}

// Thumbnail request queue - serialize requests to avoid portal conflicts on Linux
// On Windows/macOS, requests are processed in parallel for faster loading
type ThumbnailRequest = {
  type: "window" | "display";
  id: string | number;
  imgElement: HTMLImageElement;
};
const thumbnailQueue: ThumbnailRequest[] = [];
let thumbnailQueueProcessing = false;

// Track pending parallel requests to avoid duplicates
const pendingThumbnails = new Set<HTMLImageElement>();

async function processThumbnailQueue(): Promise<void> {
  if (thumbnailQueueProcessing) return;
  thumbnailQueueProcessing = true;

  // Collect all pending requests
  const requests = [...thumbnailQueue];
  thumbnailQueue.length = 0;

  // Filter out requests for elements no longer in DOM
  const validRequests = requests.filter(r => document.contains(r.imgElement));

  // Process all requests in parallel - Windows/macOS don't have portal serialization issues
  // and even on Linux, thumbnail capture uses screencopy which doesn't conflict
  const promises = validRequests.map(async (request) => {
    try {
      if (request.type === "window") {
        await loadWindowThumbnailDirect(request.id as number, request.imgElement);
      } else {
        await loadDisplayThumbnailDirect(request.id as string, request.imgElement);
      }
    } catch (error) {
      console.error(`Failed to load ${request.type} thumbnail:`, error);
    } finally {
      pendingThumbnails.delete(request.imgElement);
    }
  });

  await Promise.all(promises);

  thumbnailQueueProcessing = false;

  // Process any new requests that came in while we were processing
  if (thumbnailQueue.length > 0) {
    processThumbnailQueue();
  }
}

function queueThumbnailRequest(request: ThumbnailRequest): void {
  // Don't queue duplicates for the same element
  if (pendingThumbnails.has(request.imgElement)) return;
  
  const exists = thumbnailQueue.some(
    (r) => r.imgElement === request.imgElement
  );
  if (!exists) {
    pendingThumbnails.add(request.imgElement);
    thumbnailQueue.push(request);
    processThumbnailQueue();
  }
}

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
let formatBtnEl: HTMLButtonElement | null;
let formatDropdownEl: HTMLDivElement | null;
let modeConfigBtn: HTMLButtonElement | null;
let configViewEl: HTMLElement | null;
let outputDirInput: HTMLInputElement | null;
let browseOutputDirBtn: HTMLButtonElement | null;
let outputDirErrorEl: HTMLElement | null;
let audioSourceSelect: HTMLSelectElement | null;
let micSourceSelect: HTMLSelectElement | null;
let aecCheckbox: HTMLInputElement | null;
let aecConfigItem: HTMLElement | null;
let refreshAudioBtn: HTMLButtonElement | null;

// State
let captureMode: CaptureMode = "window";
let selectedWindow: WindowInfo | null = null;
let selectedRegion: CaptureRegion | null = null;
let selectedDisplay: MonitorInfo | null = null;
let regionSelectorWindow: WebviewWindow | null = null;
let currentState: RecordingState = "idle";
let timerInterval: number | null = null;
let recordingStartTime: number = 0;
let selectedFormat: OutputFormat = "mp4";
let defaultOutputDir: string = "";
let outputDirSaveTimeout: number | null = null;

// Thumbnail refresh state
let windowThumbnailRefreshInterval: number | null = null;
let displayThumbnailRefreshInterval: number | null = null;
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
  formatBtnEl = document.querySelector("#format-btn");
  formatDropdownEl = document.querySelector("#format-dropdown");
  modeConfigBtn = document.querySelector("#mode-config-btn");
  configViewEl = document.querySelector("#config-view");
  outputDirInput = document.querySelector("#output-dir-input");
  browseOutputDirBtn = document.querySelector("#browse-output-dir-btn");
  outputDirErrorEl = document.querySelector("#output-dir-error");
  audioSourceSelect = document.querySelector("#audio-source-select");
  micSourceSelect = document.querySelector("#mic-source-select");
  aecCheckbox = document.querySelector("#aec-checkbox");
  aecConfigItem = document.querySelector("#aec-config-item");
  refreshAudioBtn = document.querySelector("#refresh-audio-btn");

  // Set up event listeners
  closeBtn?.addEventListener("click", async () => {
    // Close region selector first if open, then close the main window
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
  
  // Also listen for window close event (e.g., from OS close button or Alt+F4)
  getCurrentWebviewWindow().onCloseRequested(async (_event) => {
    console.log("onCloseRequested fired");
    // If selector is open, close it first but don't prevent the main window close
    if (regionSelectorWindow) {
      console.log("Closing selector from onCloseRequested");
      try {
        await regionSelectorWindow.close();
      } catch (e) {
        console.warn("Failed to close selector on close request:", e);
      }
      regionSelectorWindow = null;
    }
    console.log("Allowing close to proceed");
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
  modeConfigBtn?.addEventListener("click", () => setViewMode("config"));

  // Config view handlers
  outputDirInput?.addEventListener("input", handleOutputDirInput);
  outputDirInput?.addEventListener("blur", handleOutputDirBlur);
  browseOutputDirBtn?.addEventListener("click", handleBrowseOutputDir);
  audioSourceSelect?.addEventListener("change", handleAudioConfigChange);
  micSourceSelect?.addEventListener("change", handleAudioConfigChange);
  aecCheckbox?.addEventListener("change", handleAudioConfigChange);
  refreshAudioBtn?.addEventListener("click", loadAudioSources);

  // Format selector handlers
  formatBtnEl?.addEventListener("click", toggleFormatDropdown);
  
  // Handle clicking on format options
  formatDropdownEl?.querySelectorAll(".format-option").forEach((option) => {
    option.addEventListener("click", () => {
      const value = (option as HTMLElement).dataset.value as OutputFormat;
      selectFormat(value);
    });
  });

  // Close dropdown when clicking outside
  document.addEventListener("click", (e) => {
    if (formatDropdownEl && !formatDropdownEl.classList.contains("hidden")) {
      const target = e.target as HTMLElement;
      if (!target.closest(".format-selector")) {
        closeFormatDropdown();
      }
    }
  });

  // Listen for region updates from selector window (continuous updates as user moves/resizes)
  listen<CaptureRegion>("region-updated", (event) => {
    console.log("Received region-updated:", event.payload);
    selectedRegion = event.payload;
    updateRegionDisplay();
    updateRecordButton();
  });

  // Listen for transcoding events
  listen<string>("transcoding-started", (event) => {
    console.log("Transcoding started:", event.payload);
    setStatus(`Transcoding to ${event.payload}...`);
  });

  listen<TranscodingCompleteEvent>("transcoding-complete", (event) => {
    console.log("Transcoding complete:", event.payload);
    if (!event.payload.success && event.payload.error) {
      // Transcoding failed but MP4 was saved
      setStatus(`Transcoding failed: ${event.payload.error}`, true);
    }
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

  // Initial load - check permissions first
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
  if (!appVersionEl) return;
  try {
    const version = await getVersion();
    appVersionEl.textContent = `v${version}`;
  } catch (error) {
    console.error("Failed to load app version:", error);
  }
}

// Set view mode (capture mode or config)
function setViewMode(mode: ViewMode): void {
  if (currentState !== "idle") return;

  // Update button states for all tabs
  modeWindowBtn?.classList.toggle("active", mode === "window");
  modeRegionBtn?.classList.toggle("active", mode === "region");
  modeDisplayBtn?.classList.toggle("active", mode === "display");
  modeConfigBtn?.classList.toggle("active", mode === "config");

  // Show/hide sections
  windowSelectionEl?.classList.toggle("hidden", mode !== "window");
  regionSelectionEl?.classList.toggle("hidden", mode !== "region");
  displaySelectionEl?.classList.toggle("hidden", mode !== "display");
  configViewEl?.classList.toggle("hidden", mode !== "config");

  // Show/hide controls section (hidden in config mode)
  const controlsEl = document.querySelector(".controls");
  controlsEl?.classList.toggle("hidden", mode === "config");

  // Handle switching to/from capture modes
  if (mode !== "config") {
    captureMode = mode;
  }

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

  // Load displays when switching to display mode
  if (mode === "display") {
    loadDisplays();
  }

  // Restart window refresh when switching to window mode
  if (mode === "window") {
    startWindowThumbnailRefresh();
  }

  // Update record button only for capture modes
  if (mode !== "config") {
    updateRecordButton();
  }
}

// Format dropdown functions
function toggleFormatDropdown(): void {
  if (!formatDropdownEl || !formatBtnEl) return;
  
  const isOpen = !formatDropdownEl.classList.contains("hidden");
  if (isOpen) {
    closeFormatDropdown();
  } else {
    openFormatDropdown();
  }
}

function openFormatDropdown(): void {
  if (!formatDropdownEl || !formatBtnEl) return;
  formatDropdownEl.classList.remove("hidden");
  formatBtnEl.classList.add("open");
  
  // Update selected state
  formatDropdownEl.querySelectorAll(".format-option").forEach((option) => {
    const value = (option as HTMLElement).dataset.value;
    option.classList.toggle("selected", value === selectedFormat);
  });
}

function closeFormatDropdown(): void {
  if (!formatDropdownEl || !formatBtnEl) return;
  formatDropdownEl.classList.add("hidden");
  formatBtnEl.classList.remove("open");
}

async function selectFormat(format: OutputFormat): Promise<void> {
  try {
    await invoke("set_output_format", { format });
    selectedFormat = format;
    
    // Update button display
    updateFormatButtonDisplay();
    
    console.log("Output format set to:", format);
  } catch (error) {
    console.error("Failed to set output format:", error);
    setStatus(`Failed to change format: ${error}`, true);
  }
  
  closeFormatDropdown();
}

function updateFormatButtonDisplay(): void {
  if (!formatBtnEl) return;
  
  const formatNames: Record<OutputFormat, string> = {
    mp4: "MP4",
    webm: "WebM",
    mkv: "MKV",
    quicktime: "QuickTime",
    gif: "GIF",
    apng: "APNG",
    webp: "WebP",
  };
  
  const valueEl = formatBtnEl.querySelector(".format-btn__value");
  if (valueEl) {
    valueEl.textContent = formatNames[selectedFormat];
  }
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

// Load available windows
async function loadWindows(): Promise<void> {
  if (!windowListEl) return;

  windowListEl.innerHTML = '<p class="loading">Loading windows...</p>';
  selectedWindow = null;
  updateRecordButton();

  // Stop any existing refresh interval
  stopWindowThumbnailRefresh();

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

    // Load thumbnails now that items are in DOM
    for (const win of windows) {
      const item = windowListEl.querySelector(`[data-handle="${win.handle}"]`);
      const img = item?.querySelector<HTMLImageElement>(".window-item__thumb-img");
      if (img) {
        loadWindowThumbnail(win.handle, img);
      }
    }

    // Start auto-refresh for thumbnails
    startWindowThumbnailRefresh();
  } catch (error) {
    windowListEl.innerHTML = `<p class="error">Error loading windows: ${error}</p>`;
    setStatus(`Error: ${error}`, true);
  }
}

// Start auto-refresh for window thumbnails
function startWindowThumbnailRefresh(): void {
  if (windowThumbnailRefreshInterval !== null) return;
  
  windowThumbnailRefreshInterval = window.setInterval(() => {
    if (currentState !== "idle" || captureMode !== "window") {
      return; // Don't refresh during recording or when not in window mode
    }
    refreshWindowThumbnails();
  }, 5000);
}

// Stop auto-refresh for window thumbnails
function stopWindowThumbnailRefresh(): void {
  if (windowThumbnailRefreshInterval !== null) {
    clearInterval(windowThumbnailRefreshInterval);
    windowThumbnailRefreshInterval = null;
  }
}

// Refresh window list and thumbnails (incremental update)
async function refreshWindowThumbnails(): Promise<void> {
  if (!windowListEl) return;

  try {
    const windows = await invoke<WindowInfo[]>("get_windows");
    const newHandles = new Set(windows.map(w => w.handle));

    // Get current items in the DOM
    const existingItems = windowListEl.querySelectorAll<HTMLElement>(".window-item");
    const existingHandles = new Set<number>();

    // Remove items that no longer exist
    existingItems.forEach((item) => {
      const handle = parseInt(item.dataset.handle || "0", 10);
      if (!newHandles.has(handle)) {
        item.remove();
        // Clear selection if removed window was selected
        if (selectedWindow?.handle === handle) {
          selectedWindow = null;
          updateRecordButton();
        }
      } else {
        existingHandles.add(handle);
      }
    });

    // Handle empty state
    if (windows.length === 0) {
      if (!windowListEl.querySelector(".empty")) {
        windowListEl.innerHTML = '<p class="empty">No capturable windows found</p>';
      }
      return;
    }

    // Remove empty message if present
    const emptyMsg = windowListEl.querySelector(".empty");
    if (emptyMsg) emptyMsg.remove();

    // Add new items that don't exist yet
    for (const win of windows) {
      if (!existingHandles.has(win.handle)) {
        const item = createWindowItem(win);
        windowListEl.appendChild(item);
        // Load thumbnail for the new item
        const img = item.querySelector<HTMLImageElement>(".window-item__thumb-img");
        if (img) {
          loadWindowThumbnail(win.handle, img);
        }
      }
    }

    // Refresh thumbnails for existing items
    existingHandles.forEach((handle) => {
      thumbnailCache.delete(`window:${handle}`);
      const item = windowListEl!.querySelector(`[data-handle="${handle}"]`);
      const img = item?.querySelector<HTMLImageElement>(".window-item__thumb-img");
      if (img) {
        loadWindowThumbnail(handle, img);
      }
    });
  } catch (error) {
    console.error("Error refreshing windows:", error);
  }
}

// Create a window list item element
function createWindowItem(win: WindowInfo): HTMLElement {
  const item = document.createElement("div");
  item.className = "window-item";
  item.dataset.handle = String(win.handle);

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

  // NOTE: Don't load thumbnail here - element not in DOM yet
  // Thumbnails are loaded after items are appended in loadWindows()

  return item;
}

// Load a window thumbnail (queued to serialize portal requests)
function loadWindowThumbnail(handle: number, imgElement: HTMLImageElement): void {
  const cacheKey = `window:${handle}`;
  const cached = getCachedThumbnail(cacheKey);
  
  if (cached) {
    imgElement.src = `data:image/jpeg;base64,${cached}`;
    imgElement.classList.add("loaded");
    return;
  }

  // Queue the request to avoid concurrent portal sessions
  queueThumbnailRequest({ type: "window", id: handle, imgElement });
}

// Direct thumbnail load (called from queue processor)
async function loadWindowThumbnailDirect(handle: number, imgElement: HTMLImageElement): Promise<void> {
  const cacheKey = `window:${handle}`;
  
  // Check cache again in case it was populated while queued
  const cached = getCachedThumbnail(cacheKey);
  if (cached) {
    imgElement.src = `data:image/jpeg;base64,${cached}`;
    imgElement.classList.add("loaded");
    return;
  }

  const result = await invoke<ThumbnailResponse | null>("get_window_thumbnail", {
    windowHandle: handle,
  });
  
  if (result && result.data) {
    setCachedThumbnail(cacheKey, result.data);
    imgElement.src = `data:image/jpeg;base64,${result.data}`;
    imgElement.classList.add("loaded");
  }
}

// Load available displays
async function loadDisplays(): Promise<void> {
  if (!displayListEl) return;

  displayListEl.innerHTML = '<p class="loading">Loading displays...</p>';
  selectedDisplay = null;
  updateRecordButton();

  // Stop any existing refresh interval
  stopDisplayThumbnailRefresh();

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

    // Load thumbnails now that items are in DOM
    // Use DOM traversal instead of querySelector to avoid issues with special characters in IDs
    const items = displayListEl.querySelectorAll<HTMLElement>(".display-item");
    items.forEach((item) => {
      const monitorId = item.dataset.id;
      const img = item.querySelector<HTMLImageElement>(".display-item__thumb-img");
      if (img && monitorId) {
        loadDisplayThumbnail(monitorId, img);
      }
    });

    // Start auto-refresh for thumbnails
    startDisplayThumbnailRefresh();
  } catch (error) {
    displayListEl.innerHTML = `<p class="error">Error loading displays: ${error}</p>`;
    setStatus(`Error: ${error}`, true);
  }
}

// Start auto-refresh for display thumbnails
function startDisplayThumbnailRefresh(): void {
  if (displayThumbnailRefreshInterval !== null) return;
  
  displayThumbnailRefreshInterval = window.setInterval(() => {
    if (currentState !== "idle" || captureMode !== "display") {
      return; // Don't refresh during recording or when not in display mode
    }
    refreshDisplayThumbnails();
  }, 5000);
}

// Stop auto-refresh for display thumbnails
function stopDisplayThumbnailRefresh(): void {
  if (displayThumbnailRefreshInterval !== null) {
    clearInterval(displayThumbnailRefreshInterval);
    displayThumbnailRefreshInterval = null;
  }
}

// Refresh display list and thumbnails (incremental update)
async function refreshDisplayThumbnails(): Promise<void> {
  if (!displayListEl) return;

  try {
    const displays = await invoke<MonitorInfo[]>("get_monitors");
    const newIds = new Set(displays.map(d => d.id));

    // Get current items in the DOM
    const existingItems = displayListEl.querySelectorAll<HTMLElement>(".display-item");
    const existingIds = new Set<string>();

    // Remove items that no longer exist
    existingItems.forEach((item) => {
      const id = item.dataset.id;
      if (!id || !newIds.has(id)) {
        item.remove();
        // Clear selection if removed display was selected
        if (selectedDisplay?.id === id) {
          selectedDisplay = null;
          updateRecordButton();
        }
      } else {
        existingIds.add(id);
      }
    });

    // Handle empty state
    if (displays.length === 0) {
      if (!displayListEl.querySelector(".empty")) {
        displayListEl.innerHTML = '<p class="empty">No displays found</p>';
      }
      return;
    }

    // Remove empty message if present
    const emptyMsg = displayListEl.querySelector(".empty");
    if (emptyMsg) emptyMsg.remove();

    // Add new items that don't exist yet
    for (const display of displays) {
      if (!existingIds.has(display.id)) {
        const item = createDisplayItem(display);
        displayListEl.appendChild(item);
        // Load thumbnail for the new item
        const img = item.querySelector<HTMLImageElement>(".display-item__thumb-img");
        if (img) {
          loadDisplayThumbnail(display.id, img);
        }
      }
    }

    // Refresh thumbnails for existing items
    existingIds.forEach((id) => {
      thumbnailCache.delete(`display:${id}`);
      const item = displayListEl!.querySelector(`[data-id="${id}"]`);
      const img = item?.querySelector<HTMLImageElement>(".display-item__thumb-img");
      if (img) {
        loadDisplayThumbnail(id, img);
      }
    });
  } catch (error) {
    console.error("Error refreshing displays:", error);
  }
}

// Create a display list item element
function createDisplayItem(display: MonitorInfo): HTMLElement {
  const item = document.createElement("div");
  item.className = "display-item";
  item.dataset.id = display.id;

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

  // NOTE: Don't load thumbnail here - element not in DOM yet
  // Thumbnails are loaded after items are appended in loadDisplays()

  return item;
}

// Load a display thumbnail (queued for parallel processing)
function loadDisplayThumbnail(monitorId: string, imgElement: HTMLImageElement): void {
  const cacheKey = `display:${monitorId}`;
  const cached = getCachedThumbnail(cacheKey);
  
  if (cached) {
    imgElement.src = `data:image/jpeg;base64,${cached}`;
    imgElement.classList.add("loaded");
    return;
  }

  queueThumbnailRequest({ type: "display", id: monitorId, imgElement });
}

// Direct display thumbnail load (called from queue processor)
async function loadDisplayThumbnailDirect(monitorId: string, imgElement: HTMLImageElement): Promise<void> {
  const cacheKey = `display:${monitorId}`;
  
  // Check cache again in case it was populated while queued
  const cached = getCachedThumbnail(cacheKey);
  if (cached) {
    imgElement.src = `data:image/jpeg;base64,${cached}`;
    imgElement.classList.add("loaded");
    return;
  }

  const result = await invoke<ThumbnailResponse | null>("get_display_thumbnail", {
    monitorId,
  });
  
  if (result && result.data) {
    setCachedThumbnail(cacheKey, result.data);
    imgElement.src = `data:image/jpeg;base64,${result.data}`;
    imgElement.classList.add("loaded");
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
  if (formatBtnEl) {
    formatBtnEl.disabled = disabled;
    if (disabled) {
      closeFormatDropdown();
    }
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
// Configuration Functions
// =============================================================================

// Load configuration and populate UI
async function loadConfig(): Promise<void> {
  try {
    // Load default output directory for placeholder
    defaultOutputDir = await invoke<string>("get_default_output_directory");
    
    // Load saved config
    const config = await invoke<AppConfig>("get_config");
    
    // Update output directory input
    if (outputDirInput) {
      outputDirInput.placeholder = defaultOutputDir;
      outputDirInput.value = config.output.directory || "";
    }
    
    console.log("[Config] Loaded config, default dir:", defaultOutputDir);
    
    // Load audio sources and restore selection
    await loadAudioSources();
  } catch (error) {
    console.error("[Config] Failed to load config:", error);
  }
}

// Handle input in output directory field (debounced auto-save)
function handleOutputDirInput(): void {
  // Clear existing timeout
  if (outputDirSaveTimeout !== null) {
    clearTimeout(outputDirSaveTimeout);
  }
  
  // Clear any previous error
  clearOutputDirError();
  
  // Set new timeout for auto-save (500ms debounce)
  outputDirSaveTimeout = window.setTimeout(() => {
    saveOutputDirectory();
  }, 500);
}

// Handle blur on output directory field (immediate save)
function handleOutputDirBlur(): void {
  // Clear pending debounce timeout
  if (outputDirSaveTimeout !== null) {
    clearTimeout(outputDirSaveTimeout);
    outputDirSaveTimeout = null;
  }
  
  // Save immediately on blur
  saveOutputDirectory();
}

// Save output directory to config
async function saveOutputDirectory(): Promise<void> {
  if (!outputDirInput) return;
  
  const directory = outputDirInput.value.trim();
  
  try {
    // Save to backend (validates directory if not empty)
    await invoke("save_output_directory", { 
      directory: directory || null 
    });
    
    clearOutputDirError();
    console.log("[Config] Saved output directory:", directory || "(default)");
  } catch (error) {
    showOutputDirError(String(error));
    console.error("[Config] Failed to save output directory:", error);
  }
}

// Handle browse button click
async function handleBrowseOutputDir(): Promise<void> {
  try {
    const selectedPath = await invoke<string | null>("pick_output_directory");
    
    if (selectedPath && outputDirInput) {
      outputDirInput.value = selectedPath;
      // Save immediately after picker selection
      await saveOutputDirectory();
    }
  } catch (error) {
    console.error("[Config] Failed to pick directory:", error);
    setStatus(`Failed to open folder picker: ${error}`, true);
  }
}

// Show error message for output directory
function showOutputDirError(message: string): void {
  if (outputDirErrorEl) {
    outputDirErrorEl.textContent = message;
    outputDirErrorEl.classList.remove("hidden");
  }
  outputDirInput?.classList.add("has-error");
}

// Clear output directory error
function clearOutputDirError(): void {
  if (outputDirErrorEl) {
    outputDirErrorEl.textContent = "";
    outputDirErrorEl.classList.add("hidden");
  }
  outputDirInput?.classList.remove("has-error");
}

// =============================================================================
// Audio Configuration Functions
// =============================================================================

// Load available audio sources
async function loadAudioSources(): Promise<void> {
  if (!audioSourceSelect || !micSourceSelect) return;
  
  try {
    const sources = await invoke<AudioSource[]>("get_audio_sources");
    
    // Get current audio config to preserve selection
    const audioConfig = await invoke<AudioConfig>("get_audio_config");
    
    // Group sources by type
    const inputSources = sources.filter(s => s.source_type === "input");
    const outputSources = sources.filter(s => s.source_type === "output");
    
    // Populate system audio dropdown (output sources only)
    audioSourceSelect.innerHTML = '<option value="">None (no system audio)</option>';
    for (const source of outputSources) {
      const option = document.createElement("option");
      option.value = source.id;
      option.textContent = source.name;
      audioSourceSelect.appendChild(option);
    }
    
    // Populate microphone dropdown (input sources only)
    micSourceSelect.innerHTML = '<option value="">None (no microphone)</option>';
    for (const source of inputSources) {
      const option = document.createElement("option");
      option.value = source.id;
      option.textContent = source.name;
      micSourceSelect.appendChild(option);
    }
    
    // Restore previous system audio selection if still available
    if (audioConfig.source_id) {
      audioSourceSelect.value = audioConfig.source_id;
      if (audioSourceSelect.value !== audioConfig.source_id) {
        audioSourceSelect.value = "";
      }
    }
    
    // Restore previous microphone selection if still available
    if (audioConfig.microphone_id) {
      micSourceSelect.value = audioConfig.microphone_id;
      if (micSourceSelect.value !== audioConfig.microphone_id) {
        micSourceSelect.value = "";
      }
    }
    
    // Restore AEC checkbox state
    if (aecCheckbox) {
      aecCheckbox.checked = audioConfig.echo_cancellation;
    }
    
    // Update AEC visibility (only show when mic is selected)
    updateAecVisibility();
    
    console.log("[Audio] Loaded", sources.length, "audio sources (", 
      inputSources.length, "inputs,", outputSources.length, "outputs)");
  } catch (error) {
    console.error("[Audio] Failed to load audio sources:", error);
  }
}

// Update AEC checkbox visibility based on microphone selection
function updateAecVisibility(): void {
  if (!aecConfigItem || !micSourceSelect) return;
  
  const hasMic = micSourceSelect.value !== "";
  aecConfigItem.classList.toggle("hidden", !hasMic);
}

// Handle any audio configuration change
async function handleAudioConfigChange(): Promise<void> {
  if (!audioSourceSelect || !micSourceSelect || !aecCheckbox) return;
  
  const sourceId = audioSourceSelect.value || null;
  const microphoneId = micSourceSelect.value || null;
  const echoCancellation = aecCheckbox.checked;
  
  // Update AEC visibility
  updateAecVisibility();
  
  try {
    await invoke("save_audio_config", {
      enabled: true,
      sourceId,
      microphoneId,
      echoCancellation,
    });
    console.log("[Audio] Saved config: system=", sourceId || "(none)", 
      ", mic=", microphoneId || "(none)", ", aec=", echoCancellation);
  } catch (error) {
    console.error("[Audio] Failed to save audio config:", error);
  }
}

// HTML escape helper
function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
