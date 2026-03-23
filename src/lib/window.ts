import { invoke } from "@tauri-apps/api/core";

export type AppPlatform = "macos" | "linux" | "windows";

let platformPromise: Promise<AppPlatform> | null = null;

export function getPlatform(): Promise<AppPlatform> {
  if (!platformPromise) {
    platformPromise = invoke<AppPlatform>("get_platform").catch((error) => {
      platformPromise = null;
      throw error;
    });
  }

  return platformPromise;
}

export async function applyPlatformWindowClass(): Promise<AppPlatform | null> {
  try {
    const platform = await getPlatform();
    document.body.classList.add(`platform-${platform}`);
    return platform;
  } catch (error) {
    console.warn("[Window] Failed to detect platform:", error);
    return null;
  }
}

export async function getCustomChromeWindowOptions(): Promise<{
  transparent: boolean;
  hiddenTitle?: boolean;
}> {
  const platform = await getPlatform();
  if (platform === "macos") {
    return {
      transparent: true,
      hiddenTitle: true,
    };
  }

  return {
    transparent: false,
  };
}
