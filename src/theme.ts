import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "light" | "dark" | "system";

const STORAGE_KEY = "fretec.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

export function getThemeMode(): ThemeMode {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "light";
}

export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
  }
  return mode;
}

export function applyTheme(mode: ThemeMode = getThemeMode()): void {
  const resolved = resolveTheme(mode);
  document.documentElement.dataset.theme = resolved;
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    const background = getComputedStyle(document.documentElement)
      .getPropertyValue("--md-sys-color-background")
      .trim();
    if (background) meta.setAttribute("content", background);
  }
  try {
    void getCurrentWindow().setTheme(resolved).catch(() => undefined);
  } catch {
    /* 非 Tauri 环境下忽略 */
  }
}

let initialized = false;

export function initTheme(): void {
  applyTheme();
  if (initialized) return;
  initialized = true;
  window
    .matchMedia(DARK_QUERY)
    .addEventListener("change", () => {
      if (getThemeMode() === "system") applyTheme("system");
    });
}

export function setThemeMode(mode: ThemeMode): void {
  window.localStorage.setItem(STORAGE_KEY, mode);
  applyTheme(mode);
}
