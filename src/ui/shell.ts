import { h } from "../dom";
import { icon } from "./icons";

export interface ShellRefs {
  app: HTMLDivElement;
  searchBtn: HTMLButtonElement;
  statusDot: HTMLSpanElement;
  capsule: HTMLButtonElement;
  capsuleTime: HTMLSpanElement;
  lanBtn: HTMLButtonElement;
  banner: HTMLDivElement;
  bannerText: HTMLSpanElement;
  bannerAction: HTMLButtonElement;
  bannerClose: HTMLButtonElement;
  content: HTMLElement;
  mini: HTMLDivElement;
  miniCover: HTMLImageElement;
  miniTitle: HTMLSpanElement;
  miniPlay: HTMLButtonElement;
  miniProgress: HTMLSpanElement;
  nav: HTMLElement;
  navFlow: HTMLButtonElement;
  navSettings: HTMLButtonElement;
  overlayRoot: HTMLDivElement;
  clockLayer: HTMLDivElement;
  veil: HTMLDivElement;
  mediaHost: HTMLDivElement;
}

export function buildShell(): ShellRefs {
  const searchBtn = h(
    "button",
    {
      class: "icon-btn",
      id: "search-btn",
      type: "button",
      "aria-label": "搜索",
      title: "搜索",
    },
    icon("search"),
  );

  const statusDot = h("span", { class: "status-dot", id: "status-dot" });
  const capsuleTime = h("span", { class: "capsule__time" }, "25:00");
  const capsule = h(
    "button",
    {
      class: "capsule",
      id: "capsule",
      type: "button",
      "aria-label": "展开翻页钟",
      "aria-expanded": "false",
    },
    capsuleTime,
  );
  const lanBtn = h(
    "button",
    {
      class: "icon-btn",
      id: "lan-btn",
      type: "button",
      "aria-label": "多设备互联",
      title: "多设备互联",
    },
    icon("devices"),
  );

  const topBar = h(
    "header",
    { class: "top-bar", id: "top-bar" },
    searchBtn,
    h("div", { class: "top-bar__right" }, statusDot, capsule, lanBtn),
  );

  const bannerText = h("span", { class: "login-banner__text" });
  const bannerAction = h(
    "button",
    { class: "login-banner__action", type: "button" },
    "去登录",
  );
  const bannerClose = h(
    "button",
    {
      class: "login-banner__action login-banner__close",
      type: "button",
      "aria-label": "关闭提示",
    },
    icon("close"),
  );
  bannerClose.querySelector("svg")?.setAttribute("width", "18");
  bannerClose.querySelector("svg")?.setAttribute("height", "18");
  const banner = h(
    "div",
    { class: "login-banner", id: "login-banner", role: "status", hidden: "" },
    bannerText,
    bannerAction,
    bannerClose,
  );

  const content = h("main", { class: "content", id: "content" });

  const miniCover = h("img", {
    class: "mini-player__cover",
    alt: "",
    src: "",
    referrerpolicy: "no-referrer",
  }) as HTMLImageElement;
  const miniTitle = h("span", { class: "mini-player__title" });
  const miniPlay = h(
    "button",
    {
      class: "icon-btn",
      type: "button",
      "aria-label": "播放或暂停",
    },
    icon("play"),
  ) as HTMLButtonElement;
  const miniProgress = h(
    "div",
    { class: "mini-player__progress" },
    h("span"),
  ) as HTMLDivElement;
  const mini = h(
    "div",
    { class: "mini-player", id: "mini-player", hidden: "" },
    miniCover,
    h("div", { class: "mini-player__meta" }, miniTitle),
    miniPlay,
    miniProgress,
  ) as HTMLDivElement;

  const navFlow = h(
    "button",
    {
      class: "bottom-nav__item",
      type: "button",
      role: "tab",
      "aria-selected": "true",
      "data-page": "flow",
    },
    icon("flow"),
    h("span", {}, "学习流"),
  ) as HTMLButtonElement;
  const navSettings = h(
    "button",
    {
      class: "bottom-nav__item",
      type: "button",
      role: "tab",
      "aria-selected": "false",
      "data-page": "settings",
    },
    icon("settings"),
    h("span", {}, "设置"),
  ) as HTMLButtonElement;
  const nav = h(
    "nav",
    { class: "bottom-nav", id: "bottom-nav", role: "tablist" },
    navFlow,
    navSettings,
  );

  const overlayRoot = h("div", { id: "overlay-root" });
  const clockLayer = h("div", { class: "clock-layer force-dark", id: "clock-layer" });
  const veil = h("div", { class: "veil", id: "veil" });
  const mediaHost = h("div", {
    id: "player-media",
    style: "position:fixed;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none;left:-9999px;top:0;",
  });

  const app = h(
    "div",
    { id: "app" },
    topBar,
    banner,
    content,
    mini,
    nav,
    overlayRoot,
    veil,
    clockLayer,
    mediaHost,
  ) as HTMLDivElement;

  return {
    app,
    searchBtn,
    statusDot,
    capsule,
    capsuleTime,
    lanBtn,
    banner,
    bannerText,
    bannerAction,
    bannerClose,
    content,
    mini,
    miniCover,
    miniTitle,
    miniPlay,
    miniProgress,
    nav,
    navFlow,
    navSettings,
    overlayRoot,
    clockLayer,
    veil,
    mediaHost,
  };
}
