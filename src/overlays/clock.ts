import { store } from "../store";
import { formatFlip, remainingMs } from "../util/time";
import type { TimerSnapshot } from "../types";
import { timerPause, timerResume, timerStart, timerStop } from "../api";
import { showToast } from "../ui/toast";
import { icon } from "../ui/icons";
import { h } from "../dom";

let clockEl: HTMLDivElement | null = null;
let veilEl: HTMLDivElement | null = null;
let layerEl: HTMLDivElement | null = null;
let closeBtn: HTMLButtonElement | null = null;
let stageEl: HTMLDivElement | null = null;
let hintEl: HTMLDivElement | null = null;
let controlsEl: HTMLDivElement | null = null;
let pauseBtn: HTMLButtonElement | null = null;
let stopBtn: HTMLButtonElement | null = null;
let minEl: HTMLSpanElement | null = null;
let secEl: HTMLSpanElement | null = null;
let inner: HTMLDivElement | null = null;

let open = false;
let busy = false;

function reducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function placeVeil(rect: DOMRect): void {
  if (!veilEl) return;
  veilEl.style.left = `${rect.left}px`;
  veilEl.style.top = `${rect.top}px`;
  veilEl.style.width = `${rect.width}px`;
  veilEl.style.height = `${rect.height}px`;
  veilEl.style.borderRadius = `${rect.height / 2}px`;
  veilEl.style.transform = "translate3d(0, 0, 0) scale(1, 1)";
}

function expandVeil(rect: DOMRect): void {
  if (!veilEl) return;
  const cx = rect.left + rect.width / 2;
  const cy = rect.top + rect.height / 2;
  const dx = window.innerWidth / 2 - cx;
  const dy = window.innerHeight / 2 - cy;
  const scale =
    Math.max(
      window.innerWidth / Math.max(rect.width, 1),
      window.innerHeight / Math.max(rect.height, 1),
    ) * 1.08;
  veilEl.style.borderRadius = "0px";
  veilEl.style.transform = `translate3d(${dx}px, ${dy}px, 0) scale(${scale}, ${scale})`;
}

function nextFrame(fn: () => void): void {
  requestAnimationFrame(() => requestAnimationFrame(fn));
}

function onceTransition(el: HTMLElement, fn: () => void): void {
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    el.removeEventListener("transitionend", handler);
    fn();
  };
  const handler = (event: TransitionEvent) => {
    if (event.target !== el || event.propertyName !== "transform") return;
    finish();
  };
  el.addEventListener("transitionend", handler);
  window.setTimeout(finish, 520);
}

export function initClock(layer: HTMLDivElement, veil: HTMLDivElement): void {
  layerEl = layer;
  veilEl = veil;

  closeBtn = h(
    "button",
    {
      class: "icon-btn clock__close",
      type: "button",
      "aria-label": "缩小",
      title: "缩小",
    },
    icon("chevron"),
  ) as HTMLButtonElement;

  minEl = h("span", { class: "flip-card__digits" }, "25");
  secEl = h("span", { class: "flip-card__digits" }, "00");

  stageEl = h(
    "div",
    {
      class: "clock__stage",
      role: "button",
      tabindex: "0",
      "aria-label": "开始计时",
    },
    h("div", { class: "flip-card" }, minEl),
    h("div", { class: "flip-card" }, secEl),
  ) as HTMLDivElement;

  hintEl = h("div", { class: "clock__hint" }, "轻点数字开始专注");
  pauseBtn = h(
    "button",
    { class: "tonal-button", type: "button" },
    "暂停",
  ) as HTMLButtonElement;
  stopBtn = h(
    "button",
    { class: "tonal-button", type: "button" },
    "结束",
  ) as HTMLButtonElement;
  controlsEl = h(
    "div",
    { class: "clock__controls" },
    pauseBtn,
    stopBtn,
  ) as HTMLDivElement;

  inner = h(
    "div",
    { class: "clock" },
    closeBtn,
    stageEl,
    hintEl,
    controlsEl,
  ) as HTMLDivElement;
  clockEl = inner;
  layer.append(clockEl);

  closeBtn.addEventListener("click", () => closeClock());
  stopBtn.addEventListener("click", () => {
    void timerStop()
      .then((snapshot) => {
        store.set({ timer: snapshot });
        updateClock(snapshot);
      })
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "操作失败", {
          error: true,
        }),
      );
  });
  pauseBtn.addEventListener("click", () => {
    const { timer } = store.get();
    const action =
      timer.status === "paused"
        ? timerResume
        : timer.status === "running"
          ? timerPause
          : timerStart;
    void action()
      .then((snapshot) => {
        store.set({ timer: snapshot });
        updateClock(snapshot);
      })
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "操作失败", {
          error: true,
        }),
      );
  });
  stageEl.addEventListener("click", () => {
    const { timer } = store.get();
    if (timer.status !== "idle") return;
    void timerStart()
      .then((snapshot) => {
        store.set({ timer: snapshot });
        updateClock(snapshot);
      })
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "操作失败", {
          error: true,
        }),
      );
  });
  stageEl.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      stageEl?.click();
    }
  });

  updateClock(store.get().timer);

  // 进入全屏后窗口尺寸会变化，把已经铺满的遮罩重新撑到新的视口。
  window.addEventListener("resize", () => {
    if (!open || !veilEl) return;
    const capsule = document.getElementById("capsule");
    if (capsule) expandVeil(capsule.getBoundingClientRect());
  });
}

export function isClockOpen(): boolean {
  return open;
}

export function openClock(): void {
  if (busy || open || store.get().overlay !== null) return;
  const capsule = document.getElementById("capsule");
  if (!capsule || !veilEl || !layerEl || !clockEl) return;
  const rect = capsule.getBoundingClientRect();

  busy = true;
  capsule.setAttribute("aria-expanded", "true");
  store.set({ overlay: "clock" });

  veilEl.classList.remove(
    "is-active",
    "is-animating-expand",
    "is-animating-collapse",
  );
  placeVeil(rect);
  veilEl.classList.add("is-active");

  const covered = () => {
    if (!layerEl || !clockEl) return;
    layerEl.classList.add("is-open");
    clockEl.classList.add("is-visible");
    document.body.classList.add("bars-hidden");
    open = true;
    busy = false;
  };

  if (reducedMotion()) {
    expandVeil(rect);
    covered();
    return;
  }

  veilEl.classList.add("is-animating-expand");
  nextFrame(() => {
    expandVeil(rect);
    onceTransition(veilEl!, covered);
  });
}

export function closeClock(): void {
  if (busy || !open) return;
  const capsule = document.getElementById("capsule");
  if (!capsule || !veilEl || !layerEl || !clockEl) return;
  const rect = capsule.getBoundingClientRect();
  busy = true;

  clockEl.classList.remove("is-visible");
  document.body.classList.remove("bars-hidden");
  capsule.setAttribute("aria-expanded", "false");

  const finish = () => {
    if (!veilEl || !layerEl || !clockEl) return;
    layerEl.classList.remove("is-open");
    veilEl.classList.remove("is-active", "is-animating-collapse");
    veilEl.style.width = "0px";
    veilEl.style.height = "0px";
    open = false;
    busy = false;
    store.set({ overlay: null });
  };

  if (reducedMotion()) {
    finish();
    return;
  }

  veilEl.classList.remove("is-animating-expand");
  veilEl.classList.add("is-animating-collapse");
  window.setTimeout(() => {
    if (!veilEl) return;
    placeVeil(rect);
    onceTransition(veilEl, finish);
  }, 160);
}

export function updateClock(snapshot: TimerSnapshot): void {
  if (!clockEl || !minEl || !secEl) return;
  const left = remainingMs(snapshot);
  const { minutes, seconds } = formatFlip(left);
  minEl.textContent = minutes;
  secEl.textContent = seconds;

  if (hintEl) {
    hintEl.textContent =
      snapshot.status === "paused"
        ? "已暂停"
        : snapshot.status === "running"
          ? ""
          : "轻点数字开始专注";
  }
  if (controlsEl) {
    const active = snapshot.status !== "idle";
    controlsEl.classList.toggle("is-visible", active);
  }
  if (pauseBtn) {
    pauseBtn.textContent = snapshot.status === "paused" ? "继续" : "暂停";
  }
  if (closeBtn && store.get().overlay === "clock") {
    closeBtn.setAttribute("aria-label", "缩小");
  }
}
