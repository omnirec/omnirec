import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// Application URLs
const OMNIREC_WEBSITE_URL = "https://omnirec.app";
const OMNIREC_GITHUB_URL = "https://github.com/keathmilligan/omnirec";
const OMNIREC_LICENSE_URL = "https://github.com/keathmilligan/omnirec/blob/main/LICENSE";

// ── Update info types ─────────────────────────────────────────────────────────

interface UpdateInfo {
  available: boolean;
  version?: string;
  date?: string;
  notes?: string;
}

interface DownloadProgressPayload {
  chunkLength: number;
  contentLength: number | null;
}

// ── Update UI state ───────────────────────────────────────────────────────────

let downloadedBytes = 0;
let totalBytes = 0;

function showUpdateStatus(section: string) {
  const ids = ["update-checking", "update-current", "update-error", "update-available"];
  for (const id of ids) {
    const el = document.getElementById(id);
    if (el) el.style.display = id === section ? "" : "none";
  }
  const statusEl = document.getElementById("update-status");
  if (statusEl) statusEl.style.display = "";
}

function displayUpdateAvailable(info: UpdateInfo) {
  const versionEl = document.getElementById("update-version");
  if (versionEl) versionEl.textContent = `Update available: v${info.version ?? "unknown"}`;
  const notesEl = document.getElementById("update-notes");
  if (notesEl) notesEl.textContent = info.notes ?? "";
  showUpdateStatus("update-available");
  const installBtn = document.getElementById("install-update-btn") as HTMLButtonElement | null;
  if (installBtn) installBtn.disabled = false;
  const progressWrap = document.getElementById("update-progress-wrap");
  if (progressWrap) progressWrap.style.display = "none";
}

async function runUpdateCheck(userInitiated: boolean) {
  const checkBtn = document.getElementById("check-updates-btn") as HTMLButtonElement | null;
  if (checkBtn) checkBtn.disabled = true;
  showUpdateStatus("update-checking");

  try {
    const result = await invoke<UpdateInfo>("check_for_updates");
    if (result.available) {
      displayUpdateAvailable(result);
    } else {
      if (userInitiated) {
        showUpdateStatus("update-current");
      } else {
        // Background-discovered no-update: hide status if still showing "checking"
        const statusEl = document.getElementById("update-status");
        if (statusEl) statusEl.style.display = "none";
      }
    }
  } catch (e) {
    const errorEl = document.getElementById("update-error");
    if (errorEl) {
      errorEl.textContent = `Update check failed: ${e instanceof Error ? e.message : String(e)}`;
    }
    showUpdateStatus("update-error");
  } finally {
    if (checkBtn) checkBtn.disabled = false;
  }
}

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  // Close button handler
  document.getElementById("close-btn")?.addEventListener("click", () => {
    void getCurrentWebviewWindow().destroy();
  });

  // External link handlers
  document.getElementById("about-website-link")?.addEventListener("click", (e) => {
    e.preventDefault();
    void openUrl(OMNIREC_WEBSITE_URL);
  });
  document.getElementById("about-github-link")?.addEventListener("click", (e) => {
    e.preventDefault();
    void openUrl(OMNIREC_GITHUB_URL);
  });
  document.getElementById("about-license-link")?.addEventListener("click", (e) => {
    e.preventDefault();
    void openUrl(OMNIREC_LICENSE_URL);
  });

  // Load and display version
  try {
    const version = await getVersion();
    const versionEl = document.getElementById("about-version");
    if (versionEl) versionEl.textContent = `Version ${version}`;
  } catch (error) {
    console.error("Failed to load app version:", error);
  }

  // Apply theme
  try {
    const theme = await invoke<string>("get_current_theme");
    if (theme === "light") {
      document.body.classList.add("theme-light");
    }
  } catch {
    // Theme command may not exist, use default dark theme
  }

  // ── Update UI ─────────────────────────────────────────────────────────────

  // "Check for Updates" button
  document.getElementById("check-updates-btn")?.addEventListener("click", () => {
    void runUpdateCheck(true);
  });

  // "Install & Relaunch" button
  const installBtn = document.getElementById("install-update-btn") as HTMLButtonElement | null;
  if (installBtn) {
    installBtn.addEventListener("click", async () => {
      installBtn.disabled = true;
      downloadedBytes = 0;
      totalBytes = 0;
      const progressWrap = document.getElementById("update-progress-wrap");
      if (progressWrap) progressWrap.style.display = "";
      const progressEl = document.getElementById("update-progress") as HTMLProgressElement | null;
      const labelEl = document.getElementById("update-progress-label");
      if (progressEl) progressEl.value = 0;
      if (labelEl) labelEl.textContent = "0%";
      try {
        await invoke("install_update");
        // App relaunches automatically; code below only reached on error
      } catch (e) {
        const errorEl = document.getElementById("update-error");
        if (errorEl) {
          errorEl.textContent = `Install failed: ${e instanceof Error ? e.message : String(e)}`;
        }
        showUpdateStatus("update-error");
        installBtn.disabled = false;
      }
    });
  }

  // Download progress events from Rust
  await listen<DownloadProgressPayload>("update-download-progress", (event) => {
    const { chunkLength, contentLength } = event.payload;
    if (contentLength && contentLength > 0) {
      totalBytes = contentLength;
    }
    downloadedBytes += chunkLength;

    const progressEl = document.getElementById("update-progress") as HTMLProgressElement | null;
    const labelEl = document.getElementById("update-progress-label");
    if (progressEl && totalBytes > 0) {
      const pct = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100));
      progressEl.value = pct;
      if (labelEl) labelEl.textContent = `${pct}%`;
    }
  });

  // Background update-available event (from startup check)
  await listen<UpdateInfo>("update-available", (event) => {
    displayUpdateAvailable(event.payload);
  });

  // Tray "Check for Updates" item triggers a check
  await listen("trigger-update-check", () => {
    void runUpdateCheck(true);
  });
});
