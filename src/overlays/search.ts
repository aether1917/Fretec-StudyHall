import { flowAdd, searchBili } from "../api";
import { h } from "../dom";
import { store } from "../store";
import type { SearchPage, SearchResultItem } from "../types";
import { formatDurationLabel } from "../util/time";
import { icon } from "../ui/icons";
import { showToast } from "../ui/toast";
import { closeOverlay, showOverlay } from "./manager";
import { openPlayer, openPreview } from "./player";

const PAGE_SIZE_MAX = 100;

interface SearchState {
  keyword: string;
  page: number;
  items: SearchResultItem[];
  hasMore: boolean;
  loading: boolean;
  error: string;
  token: number;
  added: Set<string>;
}

let state: SearchState | null = null;
let inputEl: HTMLInputElement | null = null;
let resultsEl: HTMLDivElement | null = null;
let countEl: HTMLDivElement | null = null;
let debounceTimer: number | null = null;
let observer: IntersectionObserver | null = null;

export function openSearchOverlay(): void {
  if (state) return;
  state = {
    keyword: "",
    page: 0,
    items: [],
    hasMore: true,
    loading: false,
    error: "",
    token: 0,
    added: new Set(
      store.get().flow.map((item) => `${item.source}:${item.sourceId}`),
    ),
  };

  const backBtn = h(
    "button",
    { class: "icon-btn", type: "button", "aria-label": "返回" },
    icon("back"),
  );
  backBtn.addEventListener("click", () => closeOverlay());

  inputEl = h("input", {
    class: "search__field",
    type: "search",
    placeholder: "搜索哔哩哔哩视频",
    "aria-label": "搜索关键词",
    autocomplete: "off",
  }) as HTMLInputElement;
  inputEl.addEventListener("input", () => onKeywordChanged());

  const header = h("div", { class: "search__header" }, backBtn, inputEl);

  countEl = h("div", { class: "search__count" });
  resultsEl = h("div", { class: "search__results" });
  resultsEl.addEventListener("scroll", () => {
    const el = resultsEl;
    if (!el || !state || !state.hasMore || state.loading) return;
    const nearBottom =
      el.scrollTop + el.clientHeight >= el.scrollHeight - el.clientHeight;
    if (nearBottom) void loadNextPage();
  });

  const root = h(
    "div",
    { class: "overlay-layer search" },
    header,
    countEl,
    resultsEl,
  );

  const opened = showOverlay("search", root, () => {
    state = null;
    inputEl = null;
    resultsEl = null;
    countEl = null;
    observer?.disconnect();
    observer = null;
  });
  if (!opened) return;

  renderResults();
  inputEl.focus();
}

function onKeywordChanged(): void {
  if (!state || !inputEl) return;
  state.keyword = inputEl.value;
  if (debounceTimer !== null) window.clearTimeout(debounceTimer);
  debounceTimer = window.setTimeout(() => {
    if (!state) return;
    state.items = [];
    state.page = 0;
    state.hasMore = true;
    state.error = "";
    state.token += 1;
    renderResults();
    if (state.keyword.trim()) void loadNextPage();
  }, 500);
}

async function loadNextPage(): Promise<void> {
  const current = state;
  if (!current || current.loading || !current.hasMore) return;
  const keyword = current.keyword.trim();
  if (!keyword) return;
  if (current.items.length >= PAGE_SIZE_MAX) {
    current.hasMore = false;
    renderResults();
    return;
  }
  current.loading = true;
  const token = current.token;
  renderResults();
  try {
    const page = current.page + 1;
    const result: SearchPage = await searchBili(keyword, page);
    if (!state || token !== state.token) return;
    state.items = [...state.items, ...result.items].slice(0, PAGE_SIZE_MAX);
    state.page = page;
    state.hasMore = result.hasMore && state.items.length < PAGE_SIZE_MAX;
    state.error = "";
  } catch (error) {
    if (!state || token !== state.token) return;
    state.error =
      error instanceof Error ? error.message : "搜索失败，请稍后重试";
    state.hasMore = false;
  } finally {
    if (state && token === state.token) {
      state.loading = false;
      renderResults();
    }
  }
}

function renderResults(): void {
  if (!resultsEl || !state) return;
  resultsEl.replaceChildren();
  if (countEl) {
    countEl.textContent = state.items.length > 0 ? `共 ${state.items.length} 条` : "";
  }

  if (!state.keyword.trim()) {
    resultsEl.append(
      h("div", { class: "search__status" }, "输入关键词开始搜索"),
    );
    return;
  }

  for (const item of state.items) {
    resultsEl.append(buildResult(item));
  }

  if (state.loading) {
    resultsEl.append(h("div", { class: "search__status" }, "搜索中…"));
  } else if (state.error) {
    const retry = h(
      "button",
      { class: "tonal-button", type: "button" },
      "重试",
    );
    retry.addEventListener("click", () => {
      if (!state) return;
      state.error = "";
      state.hasMore = true;
      void loadNextPage();
    });
    resultsEl.append(
      h(
        "div",
        { class: "search__status" },
        h("div", {}, state.error),
        h("div", { style: "margin-top:12px" }, retry),
      ),
    );
  } else if (state.items.length === 0) {
    resultsEl.append(
      h("div", { class: "search__status" }, "没有找到相关内容"),
    );
  } else if (!state.hasMore) {
    resultsEl.append(h("div", { class: "search__list-end" }, "没有更多了"));
  }
}

function buildResult(item: SearchResultItem): HTMLElement {
  const added = state?.added.has(`${item.source}:${item.sourceId}`) ?? false;
  const cover = h("img", {
    class: "search-result__cover",
    alt: "",
    loading: "lazy",
    referrerpolicy: "no-referrer",
    src: item.cover,
  }) as HTMLImageElement;
  cover.addEventListener("error", () => {
    cover.style.visibility = "hidden";
  });

  const action = h(
    "button",
    {
      class: "search-result__action",
      type: "button",
      disabled: added ? "" : undefined,
    },
    added ? "已添加" : "添加",
  ) as HTMLButtonElement;

  const row = h(
    "div",
    {
      class: "search-result",
      role: "button",
      tabindex: "0",
      "aria-label": `播放 ${item.title}`,
    },
    cover,
    h(
      "div",
      { class: "search-result__body" },
      h("div", { class: "search-result__title" }, item.title),
      h(
        "div",
        { class: "search-result__meta" },
        item.author || "未知作者",
        item.durationSec > 0
          ? ` · ${formatDurationLabel(item.durationSec)}`
          : "",
      ),
    ),
    action,
  ) as HTMLElement;

  const playResult = () => {
    if (!state) return;
    const results = state.items.slice();
    const existing = store
      .get()
      .flow.find(
        (entry) =>
          entry.source === item.source && entry.sourceId === item.sourceId,
      );
    closeOverlay();
    if (existing) {
      void openPlayer(existing.id);
    } else {
      void openPreview(item, results);
    }
  };
  row.addEventListener("click", playResult);
  row.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    playResult();
  });
  action.addEventListener("click", (event) => {
    event.stopPropagation();
    if (!state || state.added.has(`${item.source}:${item.sourceId}`)) return;
    void addItem(item, row, action);
  });
  return row;
}

async function addItem(
  item: SearchResultItem,
  _row: HTMLElement,
  action: HTMLButtonElement,
): Promise<void> {
  if (!state) return;
  const key = `${item.source}:${item.sourceId}`;
  if (state.added.has(key)) return;
  action.disabled = true;
  action.textContent = "添加中";
  try {
    const created = await flowAdd(item);
    state.added.add(key);
    store.set({ flow: [...store.get().flow, created] });
    action.textContent = "已添加";
  } catch (error) {
    action.disabled = false;
    action.textContent = "添加";
    showToast(error instanceof Error ? error.message : "添加失败", {
      error: true,
    });
  }
}
