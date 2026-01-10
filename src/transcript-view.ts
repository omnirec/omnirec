import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";

// Types
interface TranscriptionSegment {
  timestamp_secs: number;
  text: string;
}

interface TranscriptionSegmentsResponse {
  segments: TranscriptionSegment[];
  total_count: number;
}

// DOM elements
let transcriptContent: HTMLElement | null;
let closeBtn: HTMLButtonElement | null;

// State
let lastSegmentIndex = 0;
let pollingInterval: number | null = null;
let isPolling = false;
const POLL_INTERVAL_MS = 500; // Poll every 500ms

// Format timestamp as MM:SS
function formatTimestamp(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

// Create a segment element
function createSegmentElement(segment: TranscriptionSegment): HTMLElement {
  const el = document.createElement("div");
  el.className = "transcript-segment";
  el.innerHTML = `
    <span class="transcript-segment__timestamp">${formatTimestamp(segment.timestamp_secs)}</span>
    <span class="transcript-segment__text">${escapeHtml(segment.text)}</span>
  `;
  return el;
}

// Escape HTML to prevent XSS
function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Add segments to the transcript
function addSegments(segments: TranscriptionSegment[]): void {
  if (!transcriptContent || segments.length === 0) return;

  // Remove placeholder if present
  const placeholder = transcriptContent.querySelector(".transcript-placeholder");
  if (placeholder) {
    placeholder.remove();
  }

  // Add each segment
  for (const segment of segments) {
    const el = createSegmentElement(segment);
    transcriptContent.appendChild(el);
  }

  // Auto-scroll to bottom
  transcriptContent.scrollTop = transcriptContent.scrollHeight;
}

// Poll for new segments
async function pollForSegments(): Promise<void> {
  if (isPolling) return;
  isPolling = true;

  try {
    const response = await invoke<TranscriptionSegmentsResponse>("get_transcription_segments", {
      sinceIndex: lastSegmentIndex,
    });

    if (response.segments.length > 0) {
      addSegments(response.segments);
      lastSegmentIndex = response.total_count;
    }
  } catch (error) {
    console.error("[Transcript] Failed to poll segments:", error);
  } finally {
    isPolling = false;
  }
}

// Start polling for segments
function startPolling(): void {
  if (pollingInterval !== null) return;

  console.log("[Transcript] Starting polling...");
  pollingInterval = window.setInterval(pollForSegments, POLL_INTERVAL_MS);

  // Do an immediate poll
  pollForSegments();
}

// Stop polling
function stopPolling(): void {
  if (pollingInterval !== null) {
    clearInterval(pollingInterval);
    pollingInterval = null;
    console.log("[Transcript] Stopped polling");
  }
}

// Clear transcript content
function clearTranscript(): void {
  if (!transcriptContent) return;

  transcriptContent.innerHTML = '<p class="transcript-placeholder">Waiting for transcription...</p>';
  lastSegmentIndex = 0;
}

// Initialize
window.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  transcriptContent = document.querySelector("#transcript-content");
  closeBtn = document.querySelector("#close-btn");

  // Close button handler
  closeBtn?.addEventListener("click", () => {
    getCurrentWebviewWindow().close();
  });

  // Listen for recording state changes
  listen("recording-state-changed", (event) => {
    const state = event.payload as string;
    console.log("[Transcript] Recording state changed:", state);

    if (state === "recording") {
      clearTranscript();
      startPolling();
    } else {
      stopPolling();
    }
  });

  // Listen for transcript clear event (when new recording starts)
  listen("transcript-clear", () => {
    console.log("[Transcript] Clear event received");
    clearTranscript();
  });

  // Check current recording state and start polling if recording
  try {
    const state = await invoke<string>("get_recording_state");
    if (state === "recording") {
      startPolling();
    }
  } catch (error) {
    console.error("[Transcript] Failed to get recording state:", error);
  }

  // Apply theme from parent window
  try {
    const theme = await invoke<string>("get_current_theme");
    if (theme === "light") {
      document.body.classList.add("theme-light");
    }
  } catch (error) {
    console.log("[Transcript] Theme command not available, using default");
  }
});

// Cleanup on window unload
window.addEventListener("beforeunload", () => {
  stopPolling();
});
