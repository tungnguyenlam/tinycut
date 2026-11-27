import { convertFileSrc as tauriConvertFileSrc } from "@tauri-apps/api/core";
import { commands } from "./bindings";

export { commands };

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    window.__TAURI_INTERNALS__ !== undefined &&
    window.__TAURI_INTERNALS__ !== null
  );
}

export function convertFileSrc(path: string): string | null {
  const trimmed = path.trim();
  if (!isTauriRuntime() || !trimmed) {
    return null;
  }
  try {
    return tauriConvertFileSrc(trimmed);
  } catch {
    return null;
  }
}

// ============================================================================
// Browser / WebView Helpers (Theme, Clipboard & Download Fallback)
// ============================================================================

const THEME_STORAGE_KEY = "tinycut-theme";

export function detectInitialIsLight(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (saved === "light") return true;
    if (saved === "dark") return false;
  } catch {
    // Ignore storage access errors in restricted webviews
  }
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ?? false;
}

export function applyThemeToDocument(isLight: boolean, persist: boolean): void {
  if (typeof window === "undefined") return;
  const value = isLight ? "light" : "dark";
  document.documentElement.setAttribute("data-theme", value);
  if (persist) {
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, value);
    } catch {
      // Ignore storage errors
    }
  }
}

export async function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.style.position = "fixed";
  ta.style.opacity = "0";
  document.body.appendChild(ta);
  ta.select();
  document.execCommand("copy");
  document.body.removeChild(ta);
}

export function downloadTextWeb(filename: string, content: string): void {
  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
