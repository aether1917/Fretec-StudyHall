import { flowProgressSave, mediaClose, mediaPreview, mediaResolve } from "../api";
import { h } from "../dom";
import { store } from "../store";
import type { FlowItem, MediaPayload, MiniState, SearchResultItem, Source } from "../types";
import { formatTime } from "../util/time";
import { icon } from "../ui/icons";
import { showToast } from "../ui/toast";
import { closeOverlay, showOverlay } from "./manager";

const QUALITIES = [
  { value: 16, label: "360P" },
  { value: 64, label: "720P" },
  { value: 80, label: "1080P" },
] as const;

const SPEEDS = [
  { value: 0.5, label: "0.5x" },
  { value: 0.75, label: "0.75x" },
  { value: 1, label: "1.0x" },
  { value: 1.25, label: "1.25x" },
  { value: 1.5, label: "1.5x" },
  { value: 2, label: "2.0x" },
] as const;

interface PlayTarget {
  key: string;
  flowId: string | null;
  source: Source;
  sourceId: string;
  title: string;
  author: string;
  cover: string;
  durationSec: number;
}

interface Session {
  itemId: string;
  flowId: string | null;
  payload: MediaPayload;
  video: HTMLVideoElement | null;
  audio: HTMLAudioElement | null;
  quality: number;
  speed: number;
  playing: boolean;
  position: number;
  duration: number;
  lastSavedAt: number;
  lastEmitAt: number;
}

interface PlayerUi {
  stage: HTMLElement;
  current: HTMLElement;
  total: HTMLElement;
  range: HTMLInputElement;
  playBtn: HTMLButtonElement;
  qualityBtn: HTMLButtonElement;
  speedBtn: HTMLButtonElement;
  qualityMenu: HTMLElement;
  speedMenu: HTMLElement;
}

type MiniListener = (mini: MiniState | null) => void;

const listeners = new Set<MiniListener>();
let session: Session | null = null;
let overlayEl: HTMLElement | null = null;
let ui: PlayerUi | null = null;
let resolveToken = 0;
let playlist: PlayTarget[] = [];

export const playerBridge = {
  subscribe(listener: MiniListener): () => void {
    listeners.add(listener);
    listener(miniSnapshot());
    return () => {
      listeners.delete(listener);
    };
  },
  playing(): boolean {
    return session?.playing ?? false;
  },
  itemId(): string | null {
    return session?.itemId ?? null;
  },
};

function miniSnapshot(): MiniState | null {
  const current = session;
  if (!current) return null;
  return {
    itemId: current.itemId,
    title: current.payload.title,
    cover: current.payload.cover,
    playing: current.playing,
    positionSec: current.position,
    durationSec: current.duration || current.payload.durationSec,
  };
}

function emit(): void {
  const snapshot = miniSnapshot();
  for (const listener of listeners) listener(snapshot);
}

function syncFlowProgress(): void {
  const current = session;
  if (!current || !current.flowId) return;
  let changed = false;
  const next = store.get().flow.map((item) => {
    if (item.id !== current.flowId) return item;
    if (
      Math.abs(item.progress.positionSec - current.position) < 1 &&
      Math.abs(item.durationSec - (current.duration || item.durationSec)) < 1
    ) {
      return item;
    }
    changed = true;
    return {
      ...item,
      durationSec: current.duration || item.durationSec,
      progress: { positionSec: current.position, updatedAt: Date.now() },
    };
  });
  if (changed) store.set({ flow: next });
}

function mediaHost(): HTMLElement {
  const host = document.getElementById("player-media");
  if (!host) throw new Error("player media host missing");
  return host;
}

function currentTimeOf(current: Session): number {
  return current.video?.currentTime ?? current.audio?.currentTime ?? 0;
}

function durationOf(current: Session): number {
  const raw =
    current.video?.duration ??
    current.audio?.duration ??
    current.payload.durationSec;
  return Number.isFinite(raw) && raw > 0 ? raw : current.payload.durationSec;
}

function createMedia(payload: MediaPayload): {
  video: HTMLVideoElement | null;
  audio: HTMLAudioElement | null;
} {
  if (payload.kind === "audio") {
    const audio = document.createElement("audio");
    audio.preload = "auto";
    audio.src = payload.singleUrl ?? "";
    mediaHost().append(audio);
    return { video: null, audio };
  }

  if (payload.videoUrl && payload.audioUrl) {
    const video = document.createElement("video");
    video.className = "player__video";
    video.preload = "auto";
    video.muted = true;
    video.src = payload.videoUrl;
    const audio = document.createElement("audio");
    audio.preload = "auto";
    audio.src = payload.audioUrl;
    mediaHost().append(video, audio);
    return { video, audio };
  }

  const video = document.createElement("video");
  video.className = "player__video";
  video.preload = "auto";
  video.src = payload.singleUrl ?? payload.videoUrl ?? "";
  mediaHost().append(video);
  return { video, audio: null };
}

function destroyMedia(target: Session | null): void {
  if (!target) return;
  for (const element of [target.video, target.audio]) {
    if (!element) continue;
    try {
      element.pause();
    } catch {
      /* ignore */
    }
    element.removeAttribute("src");
    element.load();
    element.remove();
  }
  target.video = null;
  target.audio = null;
}

function applyRate(current: Session): void {
  if (current.video) current.video.playbackRate = current.speed;
  if (current.audio) current.audio.playbackRate = current.speed;
}

function keepTracksInSync(current: Session): void {
  if (!current.video || !current.audio || current.video.paused) return;
  const drift = Math.abs(
    current.audio.currentTime - current.video.currentTime,
  );
  if (drift > 0.3) {
    current.audio.currentTime = current.video.currentTime;
  }
}

function bindMedia(current: Session): void {
  const { video, audio } = current;

  video?.addEventListener("play", () => {
    current.playing = true;
    if (audio) void audio.play().catch(() => undefined);
    updatePlayButton();
    emit();
  });
  video?.addEventListener("pause", () => {
    current.playing = false;
    audio?.pause();
    updatePlayButton();
    emit();
    syncFlowProgress();
    void persistProgress(true);
  });
  video?.addEventListener("timeupdate", () => {
    if (!session) return;
    current.position = currentTimeOf(current);
    current.duration = durationOf(current);
    keepTracksInSync(current);
    updateProgressUi();
    if (Date.now() - current.lastEmitAt > 500) {
      current.lastEmitAt = Date.now();
      emit();
    }
    void persistProgress(false);
  });
  video?.addEventListener("ended", () => {
    current.playing = false;
    updatePlayButton();
    syncFlowProgress();
    void persistProgress(true);
    emit();
  });
  video?.addEventListener("error", () => {
    showToast("播放地址不可用，请稍后重试", { error: true });
  });

  if (audio) {
    audio.addEventListener("play", () => {
      if (current.video && current.video.paused) {
        void current.video.play().catch(() => undefined);
      }
    });
    audio.addEventListener("error", () => {
      showToast("音频地址不可用，请稍后重试", { error: true });
    });
    if (!video) {
      audio.addEventListener("timeupdate", () => {
        if (!session) return;
        current.position = currentTimeOf(current);
        current.duration = durationOf(current);
        updateProgressUi();
        if (Date.now() - current.lastEmitAt > 500) {
          current.lastEmitAt = Date.now();
          emit();
        }
        void persistProgress(false);
      });
      audio.addEventListener("pause", () => {
        current.playing = false;
        updatePlayButton();
        emit();
        syncFlowProgress();
        void persistProgress(true);
      });
      audio.addEventListener("ended", () => {
        current.playing = false;
        updatePlayButton();
        syncFlowProgress();
        void persistProgress(true);
        emit();
      });
    }
  }
}

async function persistProgress(force: boolean): Promise<void> {
  const current = session;
  if (!current || !current.flowId) return;
  const now = Date.now();
  if (!force && now - current.lastSavedAt < 1000) return;
  current.lastSavedAt = now;
  current.position = currentTimeOf(current);
  current.duration = durationOf(current);
  try {
    await flowProgressSave(current.flowId, current.position, current.duration);
  } catch {
    /* 进度保存失败不打断播放 */
  }
}

function seekTo(current: Session, seconds: number): void {
  const target = Math.max(0, Math.min(seconds, current.duration || seconds));
  if (current.video) current.video.currentTime = target;
  if (current.audio) current.audio.currentTime = target;
  current.position = target;
}

function targetOfFlow(item: FlowItem): PlayTarget {
  return {
    key: item.id,
    flowId: item.id,
    source: item.source,
    sourceId: item.sourceId,
    title: item.title,
    author: item.author,
    cover: item.cover,
    durationSec: item.durationSec,
  };
}

function targetOfSearch(item: SearchResultItem): PlayTarget {
  return {
    key: `preview:${item.source}:${item.sourceId}`,
    flowId: null,
    source: item.source,
    sourceId: item.sourceId,
    title: item.title,
    author: item.author,
    cover: item.cover,
    durationSec: item.durationSec,
  };
}

function searchItemOf(target: PlayTarget): SearchResultItem {
  return {
    source: target.source,
    sourceId: target.sourceId,
    title: target.title,
    author: target.author,
    cover: target.cover,
    durationSec: target.durationSec,
  };
}

async function startSession(target: PlayTarget, quality: number): Promise<void> {
  const payload = target.flowId
    ? await mediaResolve(target.flowId, quality)
    : await mediaPreview(searchItemOf(target), quality);
  const { video, audio } = createMedia(payload);
  const current: Session = {
    itemId: target.key,
    flowId: target.flowId,
    payload,
    video,
    audio,
    quality: payload.quality.actual,
    speed: 1,
    playing: false,
    position: payload.positionSec,
    duration: payload.durationSec,
    lastSavedAt: 0,
    lastEmitAt: 0,
  };
  session = current;
  bindMedia(current);
  applyRate(current);
  if (payload.positionSec > 1) seekTo(current, payload.positionSec);
  if (payload.notice) showToast(payload.notice, { duration: 2600 });
  emit();
}

function teardownSession(): void {
  const current = session;
  if (!current) return;
  current.position = currentTimeOf(current);
  void persistProgress(true);
  syncFlowProgress();
  destroyMedia(current);
  session = null;
  void mediaClose().catch(() => undefined);
  emit();
}

async function openTarget(target: PlayTarget, quality = 64): Promise<void> {
  if (store.get().overlay !== null) return;

  if (!session || session.itemId !== target.key) {
    teardownSession();
    try {
      await startSession(target, quality);
    } catch (error) {
      showToast(error instanceof Error ? error.message : "无法打开该内容", {
        error: true,
        duration: 3000,
      });
      return;
    }
  }

  buildOverlay();
  if (!overlayEl) return;

  const opened = showOverlay("player", overlayEl, () => {
    overlayEl = null;
    ui = null;
    if (session?.video) mediaHost().append(session.video);
    if (session?.audio) mediaHost().append(session.audio);
    syncFlowProgress();
    emit();
  });
  if (!opened) {
    overlayEl = null;
    return;
  }

  if (session?.video) ui?.stage.append(session.video);
  await play();
}

export async function openPlayer(itemId: string): Promise<void> {
  const flow = store.get().flow;
  const item = flow.find((entry) => entry.id === itemId);
  if (!item) return;
  playlist = flow.map(targetOfFlow);
  await openTarget(targetOfFlow(item), 64);
}

export async function openPreview(
  item: SearchResultItem,
  results: SearchResultItem[],
): Promise<void> {
  playlist = results.map(targetOfSearch);
  if (!playlist.some((entry) => entry.key === targetOfSearch(item).key)) {
    playlist.unshift(targetOfSearch(item));
  }
  await openTarget(targetOfSearch(item), 64);
}

export async function resumePlayer(): Promise<void> {
  if (!session || store.get().overlay !== null) return;
  buildOverlay();
  if (!overlayEl) return;
  const opened = showOverlay("player", overlayEl, () => {
    overlayEl = null;
    ui = null;
    if (session?.video) mediaHost().append(session.video);
    if (session?.audio) mediaHost().append(session.audio);
    emit();
  });
  if (!opened) {
    overlayEl = null;
    return;
  }
  if (session?.video) ui?.stage.append(session.video);
}

function renderPlaylist(panel: HTMLElement): void {
  panel.replaceChildren();
  panel.append(
    h(
      "div",
      { class: "player__playlist-heading" },
      `播放列表 · ${playlist.length} 项`,
    ),
  );
  const list = h("div", { class: "player__playlist-list" });
  for (const entry of playlist) {
    const active = session?.itemId === entry.key;
    const row = h(
      "button",
      {
        class: "player__playlist-item",
        type: "button",
        "aria-current": active ? "true" : "false",
      },
      h(
        "div",
        { class: "player__playlist-text" },
        h("div", { class: "player__playlist-name" }, entry.title),
        h(
          "div",
          { class: "player__playlist-meta" },
          `${entry.source === "bili" ? "B站" : "已移除"} · ${entry.author}`,
        ),
      ),
    ) as HTMLButtonElement;
    if (active) {
      row.append(h("span", { class: "player__playlist-active" }, "播放中"));
    }
    row.addEventListener("click", () => {
      panel.hidden = true;
      if (active) return;
      const quality = session?.quality ?? 64;
      closeOverlay();
      // 等浮层淡出、管理器清空 active 后再打开下一条。
      window.setTimeout(() => {
        void openTarget(entry, quality);
      }, 400);
    });
    list.append(row);
  }
  panel.append(list);
}

function buildOverlay(): void {
  if (overlayEl) return;
  const current = session;
  if (!current) return;

  const backBtn = h(
    "button",
    { class: "icon-btn", type: "button", "aria-label": "返回" },
    icon("back"),
  );
  backBtn.addEventListener("click", () => closeOverlay());

  const title = h("div", { class: "player__title" }, current.payload.title);
  const playlistBtn = h(
    "button",
    { class: "text-button player__playlist-btn", type: "button" },
    "列表",
  ) as HTMLButtonElement;
  const playlistPanel = h("div", {
    class: "player__playlist",
    hidden: "",
  });
  playlistBtn.addEventListener("click", () => {
    playlistPanel.hidden = !playlistPanel.hidden;
    if (!playlistPanel.hidden) renderPlaylist(playlistPanel);
  });
  const header = h(
    "div",
    { class: "player__header" },
    backBtn,
    title,
    playlistBtn,
  );

  const stage = h("div", { class: "player__stage" });
  if (current.payload.kind === "audio") {
    stage.className = "player__audio-stage";
    stage.append(
      h("img", {
        class: "player__cover",
        src: current.payload.cover,
        alt: "",
        referrerpolicy: "no-referrer",
      }),
      h("div", { class: "player__audio-title" }, current.payload.title),
      h("div", { class: "player__audio-author" }, current.payload.author),
    );
  }

  const currentLabel = h("span", { class: "player__time" }, "00:00");
  const totalLabel = h(
    "span",
    { class: "player__time player__time--total" },
    formatTime(current.duration),
  );
  const range = h("input", {
    class: "player__range",
    type: "range",
    min: "0",
    max: "1000",
    step: "1",
    value: "0",
    "aria-label": "播放进度",
  }) as HTMLInputElement;

  const playBtn = h(
    "button",
    { class: "icon-btn player__play", type: "button", "aria-label": "播放" },
    icon("play"),
  ) as HTMLButtonElement;

  const qualityBtn = h(
    "button",
    { class: "text-button", type: "button" },
    labelOfQuality(current.quality),
  ) as HTMLButtonElement;
  const speedBtn = h(
    "button",
    { class: "text-button", type: "button" },
    "1.0x",
  ) as HTMLButtonElement;

  const qualityMenu = buildMenu(
    QUALITIES.map((entry) => ({
      label: entry.label,
      value: String(entry.value),
    })),
    String(current.quality),
  );
  const speedMenu = buildMenu(
    SPEEDS.map((entry) => ({
      label: entry.label,
      value: String(entry.value),
    })),
    "1",
  );

  const qualityAnchor = h(
    "div",
    { class: "menu-anchor" },
    qualityBtn,
    qualityMenu,
  );
  const speedAnchor = h("div", { class: "menu-anchor" }, speedBtn, speedMenu);

  const progressRow = h(
    "div",
    { class: "player__progress-row" },
    currentLabel,
    range,
    totalLabel,
  );

  const controls = h(
    "div",
    { class: "player__controls" },
    progressRow,
    h(
      "div",
      { class: "player__buttons" },
      playBtn,
      h("div", { class: "player__spacer" }),
      ...(current.payload.kind === "audio" ? [] : [qualityAnchor]),
      speedAnchor,
    ),
  );

  overlayEl = h(
    "div",
    { class: "player" },
    header,
    playlistPanel,
    stage,
    controls,
  );
  ui = {
    stage,
    current: currentLabel,
    total: totalLabel,
    range,
    playBtn,
    qualityBtn,
    speedBtn,
    qualityMenu,
    speedMenu,
  };

  playBtn.addEventListener("click", () => {
    void togglePlayback();
  });
  range.addEventListener("input", () => {
    if (!session) return;
    const ratio = Number(range.value) / 1000;
    seekTo(session, ratio * (session.duration || 0));
    updateProgressUi();
  });
  qualityBtn.addEventListener("click", () => toggleMenu(qualityMenu));
  speedBtn.addEventListener("click", () => toggleMenu(speedMenu));
  qualityMenu.addEventListener("click", (event) => {
    const target = (event.target as HTMLElement).closest(
      "[data-value]",
    ) as HTMLElement | null;
    if (!target) return;
    hideMenus();
    void switchQuality(Number(target.dataset.value));
  });
  speedMenu.addEventListener("click", (event) => {
    const target = (event.target as HTMLElement).closest(
      "[data-value]",
    ) as HTMLElement | null;
    if (!target) return;
    hideMenus();
    const speed = Number(target.dataset.value);
    if (!session) return;
    session.speed = speed;
    applyRate(session);
    speedBtn.textContent = labelOfSpeed(speed);
    markMenu(speedMenu, String(speed));
  });

  document.addEventListener("pointerdown", closeMenusOutside);
  updatePlayButton();
  updateProgressUi();
}

function buildMenu(
  options: Array<{ label: string; value: string }>,
  selected: string,
): HTMLElement {
  return h(
    "div",
    { class: "menu", hidden: "" },
    ...options.map((option) =>
      h(
        "button",
        {
          class: "menu__item",
          type: "button",
          "data-value": option.value,
          "aria-checked": String(option.value === selected),
        },
        option.label,
        h("span", {}, option.value === selected ? "✓" : ""),
      ),
    ),
  );
}

function markMenu(menu: HTMLElement, value: string): void {
  for (const item of Array.from(menu.querySelectorAll(".menu__item"))) {
    const checked = (item as HTMLElement).dataset.value === value;
    item.setAttribute("aria-checked", String(checked));
    const mark = item.querySelector("span");
    if (mark) mark.textContent = checked ? "✓" : "";
  }
}

function toggleMenu(menu: HTMLElement): void {
  const willOpen = menu.hidden;
  hideMenus();
  if (willOpen) menu.hidden = false;
}

function hideMenus(): void {
  if (!ui) return;
  ui.qualityMenu.hidden = true;
  ui.speedMenu.hidden = true;
}

function closeMenusOutside(event: PointerEvent): void {
  if (!ui) return;
  const target = event.target as HTMLElement;
  if (target.closest(".menu") || target.closest(".text-button")) return;
  hideMenus();
}

function labelOfQuality(value: number): string {
  return QUALITIES.find((entry) => entry.value === value)?.label ?? "720P";
}

function labelOfSpeed(value: number): string {
  return SPEEDS.find((entry) => entry.value === value)?.label ?? "1.0x";
}

function updatePlayButton(): void {
  if (!ui || !session) return;
  ui.playBtn.replaceChildren(icon(session.playing ? "pause" : "play"));
  ui.playBtn.setAttribute("aria-label", session.playing ? "暂停" : "播放");
}

function updateProgressUi(): void {
  if (!ui || !session) return;
  const current = session;
  const duration = current.duration || current.payload.durationSec || 0;
  ui.current.textContent = formatTime(current.position);
  ui.total.textContent = formatTime(duration);
  ui.range.value = String(
    duration > 0
      ? Math.min(1000, Math.round((current.position / duration) * 1000))
      : 0,
  );
}

async function play(): Promise<void> {
  const current = session;
  if (!current) return;
  try {
    if (current.audio) await current.audio.play();
    if (current.video) await current.video.play();
    current.playing = true;
  } catch {
    current.playing = false;
  }
  updatePlayButton();
  emit();
}

export async function togglePlayback(): Promise<void> {
  const current = session;
  if (!current) return;
  if (current.playing) {
    current.video?.pause();
    current.audio?.pause();
    current.playing = false;
    updatePlayButton();
    await persistProgress(true);
    syncFlowProgress();
    emit();
    return;
  }
  await play();
}

async function switchQuality(quality: number): Promise<void> {
  const current = session;
  if (!current || current.quality === quality) return;
  const resumeAt = current.position;
  const wasPlaying = current.playing;
  const token = ++resolveToken;
  try {
    const payload = await mediaResolve(current.itemId, quality);
    if (token !== resolveToken || session !== current) return;
    destroyMedia(current);
    const { video, audio } = createMedia(payload);
    current.video = video;
    current.audio = audio;
    current.payload = { ...payload, positionSec: resumeAt };
    current.quality = payload.quality.actual;
    current.duration = payload.durationSec;
    bindMedia(current);
    applyRate(current);
    if (payload.positionSec > 1) {
      seekTo(current, Math.max(payload.positionSec, resumeAt));
    } else {
      seekTo(current, resumeAt);
    }
    if (ui) {
      ui.qualityBtn.textContent = labelOfQuality(current.quality);
      markMenu(ui.qualityMenu, String(current.quality));
      ui.total.textContent = formatTime(current.duration);
      if (current.video) ui.stage.append(current.video);
    }
    if (payload.notice) showToast(payload.notice, { duration: 2400 });
    if (wasPlaying) await play();
    updateProgressUi();
  } catch (error) {
    if (token === resolveToken) {
      showToast(error instanceof Error ? error.message : "切换画质失败", {
        error: true,
      });
    }
  }
}

export function stopPlayback(): void {
  teardownSession();
}

window.addEventListener("beforeunload", () => {
  if (session) void persistProgress(true);
});
