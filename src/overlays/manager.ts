import { store } from "../store";
import type { OverlayKind } from "../types";

export type DismissibleOverlay = Exclude<OverlayKind, "clock" | null>;

interface ActiveOverlay {
  kind: DismissibleOverlay;
  el: HTMLElement;
  onClose?: () => void;
}

let active: ActiveOverlay | null = null;

export function activeOverlay(): DismissibleOverlay | null {
  return active?.kind ?? null;
}

export function hasOverlay(): boolean {
  return active !== null || store.get().overlay === "clock";
}

export function showOverlay(
  kind: DismissibleOverlay,
  el: HTMLElement,
  onClose?: () => void,
): boolean {
  if (hasOverlay()) return false;
  el.classList.add("overlay-layer");
  document.getElementById("overlay-root")?.append(el);
  active = { kind, el, onClose };
  document.body.classList.add("bars-hidden");
  store.set({ overlay: kind });
  requestAnimationFrame(() => {
    el.classList.add("is-open");
  });
  return true;
}

export function closeOverlay(): void {
  const current = active;
  if (!current) return;
  active = null;
  current.onClose?.();
  current.el.classList.remove("is-open");
  window.setTimeout(() => current.el.remove(), 140);
  document.body.classList.remove("bars-hidden");
  store.set({ overlay: null });
}
