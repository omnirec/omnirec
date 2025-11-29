import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

// Types matching Rust structs
interface WindowInfo {
  handle: number;
  title: string;
  process_name: string;
}

type RecordingState = "idle" | "recording" | "saving";

interface RecordingResult {
  success: boolean;
  file_path: string | null;
  error: string | null;
}

// DOM Elements
let windowListEl: HTMLElement | null;
let recordBtn: HTMLButtonElement | null;
let refreshBtn: HTMLButtonElement | null;
let timerEl: HTMLElement | null;
let statusEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultPathEl: HTMLElement | null;
let openFolderBtn: HTMLButtonElement | null;

// State
let selectedWindow: WindowInfo | null = null;
let currentState: RecordingState = "idle";
let timerInterval: number | null = null;
let recordingStartTime: number = 0;

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", () => {
  windowListEl = document.querySelector("#window-list");
  recordBtn = document.querySelector("#record-btn");
  refreshBtn = document.querySelector("#refresh-btn");
  timerEl = document.querySelector("#timer");
  statusEl = document.querySelector("#status");
  resultEl = document.querySelector("#result");
  resultPathEl = document.querySelector("#result-path");
  openFolderBtn = document.querySelector("#open-folder-btn");

  // Set up event listeners
  refreshBtn?.addEventListener("click", loadWindows);
  recordBtn?.addEventListener("click", handleRecordClick);
  openFolderBtn?.addEventListener("click", handleOpenFolder);

  // Initial load
  loadWindows();
});

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
  if (!selectedWindow) {
    setStatus("Please select a window first", true);
    return;
  }

  setStatus("Starting recording...");
  disableWindowSelection(true);

  try {
    await invoke("start_recording", { windowHandle: selectedWindow.handle });
    currentState = "recording";
    updateRecordButton();
    startTimer();
    setStatus("Recording...");
  } catch (error) {
    setStatus(`Failed to start recording: ${error}`, true);
    disableWindowSelection(false);
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
  disableWindowSelection(false);
}

// Update record button state
function updateRecordButton(): void {
  if (!recordBtn) return;

  recordBtn.classList.remove("recording", "saving");

  switch (currentState) {
    case "idle":
      recordBtn.textContent = "Record";
      recordBtn.disabled = !selectedWindow;
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

// Disable/enable window selection during recording
function disableWindowSelection(disabled: boolean): void {
  if (refreshBtn) {
    refreshBtn.disabled = disabled;
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
