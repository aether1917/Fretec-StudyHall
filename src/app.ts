import { buildShell, type ShellRefs } from "./ui/shell";
import { store } from "./store";
import { showToast } from "./ui/toast";
import { formatMMSS, remainingMs } from "./util/time";
import { initClock, isClockOpen, closeClock, openClock, updateClock } from "./overlays/clock";
import { closeOverlay } from "./overlays/manager";
import { renderFlowPage } from "./views/flow";
import { renderSettingsPage } from "./views/settings";
import { openSearchOverlay } from "./overlays/search";
import { openLanOverlay } from "./overlays/lan";
import { playerBridge, resumePlayer, togglePlayback } from "./overlays/player";
import { icon } from "./ui/icons";
import {
  flowList,
  lanStatus,
  nativeBackExit,
  nativeFocusMode,
  onEvent,
  timerGet,
  timerPause,
  timerResume,
  timerStop,
} from "./api";
import type { LanStatus, TimerSnapshot } from "./types";
import { getCurrentWindow } from "@tauri-apps/api/window";

let refs: ShellRefs;
const isAndroid = /Android/i.test(navigator.userAgent);
const appWindow = getCurrentWindow();
let focusModeActive = false;
let focusModeBusy = false;

export async function boot(): Promise<void> {
  refs = buildShell();
  document.body.replaceChildren(refs.app);

  initClock(refs.clockLayer, refs.veil);
  wireShell();
  wireEvents();
  wireMini();
  if (isAndroid) {
    void onEvent("plugin:studyhall-native:backPressed", handleAndroidBack);
  }

  await loadInitialState();

  renderPage();
  window.setInterval(tick, 250);
  tick();
}

function handleAndroidBack(): void {
  if (isClockOpen()) {
    closeClock();
    return;
  }
  if (store.get().overlay !== null) {
    closeOverlay();
    return;
  }
  if (store.get().page !== "flow") {
    setPage("flow");
    return;
  }
  if (store.get().timer.status !== "idle") {
    showToast("专注中，结束计时后才能离开");
    return;
  }
  void nativeBackExit().catch(() => undefined);
}

function wireShell(): void {
  refs.searchBtn.addEventListener("click", () => {
    if (store.get().overlay !== null) return;
    if (store.get().timer.status !== "idle") {
      showToast("专注中不可添加");
      return;
    }
    openSearchOverlay();
  });

  refs.lanBtn.addEventListener("click", () => {
    if (store.get().overlay !== null) return;
    openLanOverlay();
  });

  refs.capsule.addEventListener("click", () => {
    openClock();
  });

  refs.navFlow.addEventListener("click", () => setPage("flow"));
  refs.navSettings.addEventListener("click", () => setPage("settings"));

  window.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (isClockOpen()) {
      closeClock();
      return;
    }
    if (store.get().overlay !== null) closeOverlay();
  });

  store.subscribe((state) => {
    updateNav(state.page);
  });

  let lastFlow = store.get().flow;
  store.subscribe((state) => {
    if (state.flow === lastFlow) return;
    lastFlow = state.flow;
    if (state.page === "flow") renderFlowPage(refs.content);
  });
}

function setPage(page: "flow" | "settings"): void {
  if (store.get().page === page) return;
  store.set({ page, highlight: null });
  renderPage();
}

function updateNav(page: "flow" | "settings"): void {
  refs.navFlow.setAttribute("aria-selected", String(page === "flow"));
  refs.navSettings.setAttribute("aria-selected", String(page === "settings"));
}

function renderPage(): void {
  const state = store.get();
  updateNav(state.page);
  if (state.page === "flow") {
    renderFlowPage(refs.content);
  } else {
    renderSettingsPage(refs.content);
  }
}

function tick(): void {
  const state = store.get();
  const timer = state.timer;
  refs.capsuleTime.textContent = formatMMSS(remainingMs(timer));
  refs.capsule.dataset.phase =
    timer.status === "idle" ? "idle" : timer.phase;
  updateClock(timer);
  void syncFocusMode(timer);
}

async function syncFocusMode(timer: TimerSnapshot): Promise<void> {
  const active = timer.status === "running" || timer.status === "paused";
  if (active === focusModeActive || focusModeBusy) return;
  focusModeBusy = true;
  focusModeActive = active;
  try {
    if (isAndroid) {
      await nativeFocusMode(active);
    } else {
      await appWindow.setFullscreen(active);
      await appWindow.setAlwaysOnTop(active);
    }
  } catch {
    /* 权限被拒绝时保持当前窗口状态 */
  } finally {
    focusModeBusy = false;
  }
}

async function loadInitialState(): Promise<void> {
  try {
    const [flow, timer] = await Promise.all([flowList(), timerGet()]);
    store.set({ flow, timer });
    void syncFocusMode(timer);
  } catch (error) {
    showToast(
      error instanceof Error ? error.message : "初始化失败，请重启应用",
      { error: true, duration: 4000 },
    );
  }
  void lanStatus()
    .then((lan) => store.set({ lan }))
    .catch(() => {
      /* 局域网不可用时保持静默，互联页会提示 */
    });
}

function wireEvents(): void {
  void onEvent<TimerSnapshot>("timer://state", (snapshot) => {
    store.set({ timer: snapshot });
    updateClock(snapshot);
  });

  void onEvent<LanStatus>("lan://status", (lan) => {
    store.set({ lan });
  });

  void onEvent<void>("ui://open-clock", () => {
    openClock();
  });

  void onEvent<{ action: string }>(
    "plugin:studyhall-native:timerAction",
    ({ action }) => {
      void handleNativeTimerAction(action);
    },
  );
}

async function handleNativeTimerAction(action: string): Promise<void> {
  try {
    const snapshot =
      action === "pause"
        ? await timerPause()
        : action === "resume"
          ? await timerResume()
          : action === "stop"
            ? await timerStop()
            : null;
    if (snapshot) {
      store.set({ timer: snapshot });
      updateClock(snapshot);
    }
  } catch {
    /* 通知操作失败时保持界面状态，等待下一次同步 */
  }
}

function wireMini(): void {
  playerBridge.subscribe((mini) => {
    store.set({ mini });
    const { mini: el } = refs;
    if (!mini) {
      el.hidden = true;
      refs.app.style.setProperty("--app-mini-offset", "0px");
      return;
    }
    el.hidden = false;
    refs.app.style.setProperty(
      "--app-mini-offset",
      "var(--md-sys-size-mini-player)",
    );
    if (refs.miniCover.getAttribute("src") !== mini.cover) {
      refs.miniCover.src = mini.cover;
    }
    refs.miniTitle.textContent = mini.title;
    refs.miniPlay.replaceChildren(icon(mini.playing ? "pause" : "play"));
  });

  refs.miniPlay.addEventListener("click", (event) => {
    event.stopPropagation();
    void togglePlayback();
  });

  refs.mini.addEventListener("click", () => {
    const mini = store.get().mini;
    if (!mini) return;
    void resumePlayer();
  });
}
