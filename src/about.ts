import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// Application URLs
const OMNIREC_WEBSITE_URL = "https://omnirec.app";
const OMNIREC_GITHUB_URL = "https://github.com/keathmilligan/omnirec";
const OMNIREC_LICENSE_URL = "https://github.com/keathmilligan/omnirec/blob/main/LICENSE";

// DOM elements
let aboutVersionEl: HTMLElement | null;
let aboutWebsiteLink: HTMLAnchorElement | null;
let aboutGithubLink: HTMLAnchorElement | null;
let aboutLicenseLink: HTMLAnchorElement | null;
let closeBtn: HTMLButtonElement | null;

// Initialize on DOM load
window.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  aboutVersionEl = document.querySelector("#about-version");
  aboutWebsiteLink = document.querySelector("#about-website-link");
  aboutGithubLink = document.querySelector("#about-github-link");
  aboutLicenseLink = document.querySelector("#about-license-link");
  closeBtn = document.querySelector("#close-btn");

  // Close button handler
  closeBtn?.addEventListener("click", () => {
    getCurrentWebviewWindow().close();
  });

  // Link handlers
  aboutWebsiteLink?.addEventListener("click", (e) => {
    e.preventDefault();
    openUrl(OMNIREC_WEBSITE_URL);
  });
  aboutGithubLink?.addEventListener("click", (e) => {
    e.preventDefault();
    openUrl(OMNIREC_GITHUB_URL);
  });
  aboutLicenseLink?.addEventListener("click", (e) => {
    e.preventDefault();
    openUrl(OMNIREC_LICENSE_URL);
  });

  // Load and display version
  try {
    const version = await getVersion();
    if (aboutVersionEl) {
      aboutVersionEl.textContent = `Version ${version}`;
    }
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
    console.log("[About] Theme command not available, using default");
  }
});
