import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";

// Types matching Rust structs
type ThemeMode = "auto" | "light" | "dark";

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

interface TranscriptionConfig {
  enabled: boolean;
  model: string;
  show_transcript_window: boolean;
}

interface ModelInfo {
  id: string;
  display_name: string;
  size_bytes: number;
  size_display: string;
  description: string;
  english_only: boolean;
  downloaded: boolean;
}

interface ModelStatus {
  model: string;
  display_name: string;
  path: string;
  exists: boolean;
  file_size: number | null;
  expected_size: number;
  size_display: string;
}

interface DownloadProgress {
  model: string;
  bytes_downloaded: number;
  total_bytes: number;
  percentage: number;
  status: "downloading" | "completed" | "cancelled" | "error";
  error: string | null;
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
  appearance: {
    theme: ThemeMode;
  };
}

// DOM elements
let closeBtn: HTMLButtonElement | null;
let outputDirInput: HTMLInputElement | null;
let browseOutputDirBtn: HTMLButtonElement | null;
let outputDirErrorEl: HTMLElement | null;
let audioSourceSelect: HTMLSelectElement | null;
let micSourceSelect: HTMLSelectElement | null;
let aecCheckbox: HTMLInputElement | null;
let aecConfigItem: HTMLElement | null;
let refreshAudioBtn: HTMLButtonElement | null;
let themeSelect: HTMLSelectElement | null;
let macosSystemAudioCheckbox: HTMLInputElement | null;
let macosSystemAudioConfigItem: HTMLElement | null;
let macosSystemAudioHint: HTMLElement | null;
let audioSourceConfigItem: HTMLElement | null;
let transcriptionCheckbox: HTMLInputElement | null;
let transcriptionConfigItem: HTMLElement | null;
let modelConfigItem: HTMLElement | null;
let modelSelect: HTMLSelectElement | null;
let modelInfo: HTMLElement | null;
let modelStatus: HTMLElement | null;
let modelDownloadBtn: HTMLButtonElement | null;
let modelCancelBtn: HTMLButtonElement | null;
let modelProgressContainer: HTMLElement | null;
let modelProgressFill: HTMLElement | null;
let modelProgressText: HTMLElement | null;
let showTranscriptConfigItem: HTMLElement | null;
let showTranscriptCheckbox: HTMLInputElement | null;

// State
let defaultOutputDir = "";
let outputDirSaveTimeout: number | null = null;
let currentPlatform: "macos" | "linux" | "windows" = "linux";
let macosSystemAudioAvailable = false;
let isModelDownloading = false;
let currentThemeMode: ThemeMode = "auto";
let systemThemeMediaQuery: MediaQueryList | null = null;

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  closeBtn = document.querySelector("#close-btn");
  outputDirInput = document.querySelector("#output-dir-input");
  browseOutputDirBtn = document.querySelector("#browse-output-dir-btn");
  outputDirErrorEl = document.querySelector("#output-dir-error");
  audioSourceSelect = document.querySelector("#audio-source-select");
  micSourceSelect = document.querySelector("#mic-source-select");
  aecCheckbox = document.querySelector("#aec-checkbox");
  aecConfigItem = document.querySelector("#aec-config-item");
  refreshAudioBtn = document.querySelector("#refresh-audio-btn");
  themeSelect = document.querySelector("#theme-select");
  macosSystemAudioCheckbox = document.querySelector("#macos-system-audio-checkbox");
  macosSystemAudioConfigItem = document.querySelector("#macos-system-audio-config-item");
  macosSystemAudioHint = document.querySelector("#macos-system-audio-hint");
  audioSourceConfigItem = document.querySelector("#audio-source-config-item");
  transcriptionCheckbox = document.querySelector("#transcription-checkbox");
  transcriptionConfigItem = document.querySelector("#transcription-config-item");
  modelConfigItem = document.querySelector("#model-config-item");
  modelSelect = document.querySelector("#model-select");
  modelInfo = document.querySelector("#model-info");
  modelStatus = document.querySelector("#model-status");
  modelDownloadBtn = document.querySelector("#model-download-btn");
  modelCancelBtn = document.querySelector("#model-cancel-btn");
  modelProgressContainer = document.querySelector("#model-progress-container");
  modelProgressFill = document.querySelector("#model-progress-fill");
  modelProgressText = document.querySelector("#model-progress-text");
  showTranscriptConfigItem = document.querySelector("#show-transcript-config-item");
  showTranscriptCheckbox = document.querySelector("#show-transcript-checkbox");

  // Close button handler
  closeBtn?.addEventListener("click", () => {
    getCurrentWebviewWindow().close();
  });

  // Config event handlers
  outputDirInput?.addEventListener("input", handleOutputDirInput);
  outputDirInput?.addEventListener("blur", handleOutputDirBlur);
  browseOutputDirBtn?.addEventListener("click", handleBrowseOutputDir);
  audioSourceSelect?.addEventListener("change", handleAudioConfigChange);
  micSourceSelect?.addEventListener("change", handleAudioConfigChange);
  aecCheckbox?.addEventListener("change", handleAudioConfigChange);
  refreshAudioBtn?.addEventListener("click", loadAudioSources);
  themeSelect?.addEventListener("change", handleThemeChange);
  macosSystemAudioCheckbox?.addEventListener("change", handleMacosSystemAudioChange);
  transcriptionCheckbox?.addEventListener("change", handleTranscriptionChange);
  modelSelect?.addEventListener("change", handleModelChange);
  modelDownloadBtn?.addEventListener("click", handleModelDownload);
  modelCancelBtn?.addEventListener("click", handleModelCancel);
  showTranscriptCheckbox?.addEventListener("change", handleShowTranscriptChange);

  // Listen for model download progress events
  listen<DownloadProgress>("model-download-progress", (event) => {
    handleDownloadProgress(event.payload);
  });

  // Load config
  loadConfig();
});

// =============================================================================
// Configuration Functions
// =============================================================================

async function loadConfig(): Promise<void> {
  try {
    defaultOutputDir = await invoke<string>("get_default_output_directory");
    const config = await invoke<AppConfig>("get_config");

    if (outputDirInput) {
      outputDirInput.placeholder = defaultOutputDir;
      outputDirInput.value = config.output.directory || "";
    }

    const themeMode = config.appearance?.theme || "auto";
    initTheme(themeMode);

    if (themeSelect) {
      themeSelect.value = themeMode;
    }

    console.log("[Config] Loaded config, default dir:", defaultOutputDir, ", theme:", themeMode);

    await loadAudioSources();
  } catch (error) {
    console.error("[Config] Failed to load config:", error);
  }
}

function handleOutputDirInput(): void {
  if (outputDirSaveTimeout !== null) {
    clearTimeout(outputDirSaveTimeout);
  }
  clearOutputDirError();
  outputDirSaveTimeout = window.setTimeout(() => {
    saveOutputDirectory();
  }, 500);
}

function handleOutputDirBlur(): void {
  if (outputDirSaveTimeout !== null) {
    clearTimeout(outputDirSaveTimeout);
    outputDirSaveTimeout = null;
  }
  saveOutputDirectory();
}

async function saveOutputDirectory(): Promise<void> {
  if (!outputDirInput) return;
  const directory = outputDirInput.value.trim();
  try {
    await invoke("save_output_directory", { directory: directory || null });
    clearOutputDirError();
    console.log("[Config] Saved output directory:", directory || "(default)");
  } catch (error) {
    showOutputDirError(String(error));
    console.error("[Config] Failed to save output directory:", error);
  }
}

async function handleBrowseOutputDir(): Promise<void> {
  try {
    const selectedPath = await invoke<string | null>("pick_output_directory");
    if (selectedPath && outputDirInput) {
      outputDirInput.value = selectedPath;
      await saveOutputDirectory();
    }
  } catch (error) {
    console.error("[Config] Failed to pick directory:", error);
  }
}

function showOutputDirError(message: string): void {
  if (outputDirErrorEl) {
    outputDirErrorEl.textContent = message;
    outputDirErrorEl.classList.remove("hidden");
  }
  outputDirInput?.classList.add("has-error");
}

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

async function loadAudioSources(): Promise<void> {
  if (!audioSourceSelect || !micSourceSelect) return;

  try {
    currentPlatform = await invoke<"macos" | "linux" | "windows">("get_platform");
    macosSystemAudioAvailable = await invoke<boolean>("is_system_audio_available");

    const sources = await invoke<AudioSource[]>("get_audio_sources");
    const audioConfig = await invoke<AudioConfig>("get_audio_config");

    const inputSources = sources.filter(s => s.source_type === "input");
    const outputSources = sources.filter(s => s.source_type === "output");

    if (currentPlatform === "macos") {
      audioSourceConfigItem?.classList.add("hidden");
      macosSystemAudioConfigItem?.classList.remove("hidden");

      if (macosSystemAudioCheckbox) {
        if (macosSystemAudioAvailable) {
          macosSystemAudioCheckbox.disabled = false;
          macosSystemAudioCheckbox.checked = audioConfig.source_id === "system";
        } else {
          macosSystemAudioCheckbox.disabled = true;
          macosSystemAudioCheckbox.checked = false;
        }
      }

      if (macosSystemAudioHint) {
        if (macosSystemAudioAvailable) {
          macosSystemAudioHint.textContent = "Capture all system audio during recording";
        } else {
          macosSystemAudioHint.textContent = "Requires macOS 13 (Ventura) or later";
          macosSystemAudioHint.classList.add("config-item__hint--warning");
        }
      }
    } else {
      audioSourceConfigItem?.classList.remove("hidden");
      macosSystemAudioConfigItem?.classList.add("hidden");

      audioSourceSelect.innerHTML = '<option value="">None (no system audio)</option>';
      for (const source of outputSources) {
        const option = document.createElement("option");
        option.value = source.id;
        option.textContent = source.name;
        audioSourceSelect.appendChild(option);
      }

      if (audioConfig.source_id && audioConfig.source_id !== "system") {
        audioSourceSelect.value = audioConfig.source_id;
        if (audioSourceSelect.value !== audioConfig.source_id) {
          audioSourceSelect.value = "";
        }
      }
    }

    micSourceSelect.innerHTML = '<option value="">None (no microphone)</option>';
    for (const source of inputSources) {
      const option = document.createElement("option");
      option.value = source.id;
      option.textContent = source.name;
      micSourceSelect.appendChild(option);
    }

    if (audioConfig.microphone_id) {
      micSourceSelect.value = audioConfig.microphone_id;
      if (micSourceSelect.value !== audioConfig.microphone_id) {
        micSourceSelect.value = "";
      }
    }

    if (aecCheckbox) {
      aecCheckbox.checked = audioConfig.echo_cancellation;
    }

    updateAecVisibility();
    updateTranscriptionVisibility();
    await loadTranscriptionConfig();

    console.log("[Audio] Platform:", currentPlatform, ", macOS system audio available:", macosSystemAudioAvailable);
    console.log("[Audio] Loaded", sources.length, "audio sources (",
      inputSources.length, "inputs,", outputSources.length, "outputs)");
  } catch (error) {
    console.error("[Audio] Failed to load audio sources:", error);
  }
}

function updateAecVisibility(): void {
  if (!aecConfigItem || !micSourceSelect) return;
  const hasMic = micSourceSelect.value !== "";
  aecConfigItem.classList.toggle("hidden", !hasMic);
}

async function handleAudioConfigChange(): Promise<void> {
  if (!audioSourceSelect || !micSourceSelect || !aecCheckbox) return;

  const sourceId = audioSourceSelect.value || null;
  const microphoneId = micSourceSelect.value || null;
  const echoCancellation = aecCheckbox.checked;

  updateAecVisibility();
  updateTranscriptionVisibility();

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

async function handleMacosSystemAudioChange(): Promise<void> {
  if (!macosSystemAudioCheckbox) return;
  const enabled = macosSystemAudioCheckbox.checked;

  try {
    await invoke("save_audio_config", {
      enabled: true,
      sourceId: enabled ? "system" : null,
      microphoneId: micSourceSelect?.value || null,
      echoCancellation: aecCheckbox?.checked ?? false,
    });
    console.log("[Audio] macOS system audio:", enabled ? "enabled" : "disabled");
    updateTranscriptionVisibility();
  } catch (error) {
    console.error("[Audio] Failed to save macOS audio config:", error);
  }
}

function updateTranscriptionVisibility(): void {
  let hasSystemAudio = false;
  if (currentPlatform === "macos") {
    hasSystemAudio = macosSystemAudioCheckbox?.checked ?? false;
  } else {
    hasSystemAudio = (audioSourceSelect?.value ?? "") !== "";
  }

  if (transcriptionConfigItem) {
    transcriptionConfigItem.classList.toggle("hidden", !hasSystemAudio);
  }

  const transcriptionEnabled = transcriptionCheckbox?.checked ?? false;
  const showModelConfig = hasSystemAudio && transcriptionEnabled;
  if (modelConfigItem) {
    modelConfigItem.classList.toggle("hidden", !showModelConfig);
  }

  if (showTranscriptConfigItem) {
    showTranscriptConfigItem.classList.toggle("hidden", !showModelConfig);
  }
}

async function handleTranscriptionChange(): Promise<void> {
  if (!transcriptionCheckbox) return;
  const enabled = transcriptionCheckbox.checked;

  updateTranscriptionVisibility();

  if (enabled) {
    await loadAvailableModels();
    await updateModelStatus();
  }

  try {
    await invoke("save_transcription_config", { enabled });
    console.log("[Transcription] Saved config: enabled=", enabled);
  } catch (error) {
    console.error("[Transcription] Failed to save config:", error);
  }
}

async function loadTranscriptionConfig(): Promise<void> {
  try {
    const config = await invoke<TranscriptionConfig>("get_transcription_config");
    if (transcriptionCheckbox) {
      transcriptionCheckbox.checked = config.enabled;
    }

    if (showTranscriptCheckbox) {
      showTranscriptCheckbox.checked = config.show_transcript_window;
    }

    await loadAvailableModels();
    if (modelSelect && config.model) {
      modelSelect.value = config.model;
    }

    await updateModelStatus();
    updateTranscriptionVisibility();

    console.log("[Transcription] Loaded config: enabled=", config.enabled, "model=", config.model, "show_transcript_window=", config.show_transcript_window);
  } catch (error) {
    console.error("[Transcription] Failed to load config:", error);
  }
}

async function handleShowTranscriptChange(): Promise<void> {
  if (!showTranscriptCheckbox) return;
  const show = showTranscriptCheckbox.checked;

  try {
    await invoke("save_transcription_config", {
      enabled: transcriptionCheckbox?.checked ?? false,
      showTranscriptWindow: show,
    });
    console.log("[Transcription] Show transcript window:", show);
  } catch (error) {
    console.error("[Transcription] Failed to save show transcript setting:", error);
  }
}

// =============================================================================
// Model Management Functions
// =============================================================================

async function loadAvailableModels(): Promise<void> {
  if (!modelSelect) return;

  try {
    const models = await invoke<ModelInfo[]>("list_available_models");
    modelSelect.innerHTML = "";
    for (const model of models) {
      const option = document.createElement("option");
      option.value = model.id;
      option.textContent = model.description;
      modelSelect.appendChild(option);
    }
    console.log("[Model] Loaded", models.length, "available models");
  } catch (error) {
    console.error("[Model] Failed to load models:", error);
    modelSelect.innerHTML = '<option value="">Failed to load models</option>';
  }
}

async function updateModelStatus(): Promise<void> {
  if (!modelSelect || !modelStatus || !modelDownloadBtn || !modelCancelBtn) return;

  const selectedModel = modelSelect.value;
  if (!selectedModel) {
    if (modelInfo) modelInfo.textContent = "";
    modelStatus.textContent = "";
    modelDownloadBtn.classList.add("hidden");
    modelCancelBtn.classList.add("hidden");
    return;
  }

  try {
    const status = await invoke<ModelStatus>("get_model_status", { model: selectedModel });

    if (modelInfo) {
      modelInfo.textContent = `${status.model} (${status.size_display})`;
    }

    if (isModelDownloading) {
      modelStatus.textContent = "Downloading...";
      modelStatus.className = "model-status model-status--downloading";
      modelDownloadBtn.classList.add("hidden");
      modelCancelBtn.classList.remove("hidden");
      modelSelect.disabled = true;
    } else if (status.exists) {
      modelStatus.textContent = "Downloaded";
      modelStatus.className = "model-status model-status--downloaded";
      modelDownloadBtn.classList.add("hidden");
      modelCancelBtn.classList.add("hidden");
      modelSelect.disabled = false;
    } else {
      modelStatus.textContent = "Not downloaded";
      modelStatus.className = "model-status model-status--not-downloaded";
      modelDownloadBtn.classList.remove("hidden");
      modelCancelBtn.classList.add("hidden");
      modelSelect.disabled = false;
    }
  } catch (error) {
    console.error("[Model] Failed to get status:", error);
    if (modelInfo) modelInfo.textContent = "";
    modelStatus.textContent = "Error checking status";
    modelStatus.className = "model-status model-status--error";
    modelDownloadBtn.classList.add("hidden");
    modelCancelBtn.classList.add("hidden");
  }
}

async function handleModelChange(): Promise<void> {
  if (!modelSelect) return;
  const selectedModel = modelSelect.value;
  console.log("[Model] Selection changed to:", selectedModel);
  await updateModelStatus();

  try {
    await invoke("save_transcription_config", {
      enabled: transcriptionCheckbox?.checked ?? false,
      model: selectedModel,
    });
    console.log("[Model] Saved model selection:", selectedModel);
  } catch (error) {
    console.error("[Model] Failed to save selection:", error);
  }
}

async function handleModelDownload(): Promise<void> {
  if (!modelSelect) return;
  const selectedModel = modelSelect.value;
  if (!selectedModel) return;

  console.log("[Model] Starting download:", selectedModel);
  isModelDownloading = true;

  if (modelProgressContainer) {
    modelProgressContainer.classList.remove("hidden");
  }
  if (modelProgressFill) {
    modelProgressFill.style.width = "0%";
  }
  if (modelProgressText) {
    modelProgressText.textContent = "0%";
  }

  await updateModelStatus();

  try {
    await invoke("download_model", { model: selectedModel });
  } catch (error) {
    console.error("[Model] Download failed:", error);
    isModelDownloading = false;
    if (modelProgressContainer) {
      modelProgressContainer.classList.add("hidden");
    }
    await updateModelStatus();
  }
}

async function handleModelCancel(): Promise<void> {
  console.log("[Model] Cancelling download");
  try {
    await invoke("cancel_download");
  } catch (error) {
    console.error("[Model] Failed to cancel download:", error);
  }
}

function handleDownloadProgress(progress: DownloadProgress): void {
  console.log("[Model] Progress:", progress.percentage.toFixed(1) + "%", progress.status);

  if (modelProgressFill) {
    modelProgressFill.style.width = `${progress.percentage}%`;
  }
  if (modelProgressText) {
    const downloadedMB = (progress.bytes_downloaded / (1024 * 1024)).toFixed(1);
    const totalMB = (progress.total_bytes / (1024 * 1024)).toFixed(1);
    modelProgressText.textContent = `${progress.percentage.toFixed(0)}% (${downloadedMB}/${totalMB} MB)`;
  }

  if (progress.status === "completed") {
    console.log("[Model] Download completed");
    isModelDownloading = false;
    if (modelProgressContainer) {
      modelProgressContainer.classList.add("hidden");
    }
    loadAvailableModels().then(() => {
      if (modelSelect) {
        modelSelect.value = progress.model;
      }
      updateModelStatus();
    });
  } else if (progress.status === "cancelled") {
    console.log("[Model] Download cancelled");
    isModelDownloading = false;
    if (modelProgressContainer) {
      modelProgressContainer.classList.add("hidden");
    }
    updateModelStatus();
  } else if (progress.status === "error") {
    console.error("[Model] Download error:", progress.error);
    isModelDownloading = false;
    if (modelProgressContainer) {
      modelProgressContainer.classList.add("hidden");
    }
    updateModelStatus();
  }
}

// =============================================================================
// Theme Functions
// =============================================================================

function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function getEffectiveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "auto") {
    return getSystemTheme();
  }
  return mode;
}

function applyTheme(theme: "light" | "dark"): void {
  const body = document.body;
  body.classList.remove("theme-light", "theme-dark");
  body.classList.add(`theme-${theme}`);
  console.log("[Theme] Applied theme:", theme);
}

function initTheme(mode: ThemeMode): void {
  currentThemeMode = mode;
  const effectiveTheme = getEffectiveTheme(mode);
  applyTheme(effectiveTheme);

  systemThemeMediaQuery = window.matchMedia("(prefers-color-scheme: light)");
  systemThemeMediaQuery.addEventListener("change", handleSystemThemeChange);
}

function handleSystemThemeChange(): void {
  if (currentThemeMode === "auto") {
    const effectiveTheme = getEffectiveTheme("auto");
    applyTheme(effectiveTheme);
    console.log("[Theme] System theme changed, applied:", effectiveTheme);
  }
}

async function handleThemeChange(): Promise<void> {
  if (!themeSelect) return;

  const newMode = themeSelect.value as ThemeMode;
  currentThemeMode = newMode;

  const effectiveTheme = getEffectiveTheme(newMode);
  applyTheme(effectiveTheme);

  try {
    await invoke("save_theme", { theme: newMode });
    console.log("[Theme] Saved theme mode:", newMode);
  } catch (error) {
    console.error("[Theme] Failed to save theme:", error);
  }
}
