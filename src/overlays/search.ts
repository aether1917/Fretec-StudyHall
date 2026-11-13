import { flowAdd, searchBili, searchNetease } from "../api";
import { h } from "../dom";
import { store } from "../store";
import type { SearchPage, SearchResultItem, Source } from "../types";
import { formatDurationLabel } from "../util/time";
import { icon } from "../ui/icons";
import { showToast } from "../ui/toast";
import { closeOverlay, showOverlay } from "./manager";

const PAGE_SIZE_MAX = 100;

interface SearchState {
  source: Source;
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
    source: "bili",
    keyword: "",
    page: 0,
    items: [],
    hasMore: true,
    loading: false,
    error: "",
    token: 0,
    added: new Set(
      store
        .get()
        .flow.filter((item) => item.source === "bili")
        .map((item) => item.sourceId),
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
    placeholder: "搜索哔哩哔哩视频或网易云音乐",
    "aria-label": "搜索关键词",
    autocomplete: "off",
  }) as HTMLInputElement;
  inputEl.addEventListener("input", () => onKeywordChanged());

  const header = h("div", { class: "search__header" }, backBtn, inputEl);

  const sourceTabs = h(
    "div",
    { class: "search__sources" },
    sourceTab("bili", "哔哩哔哩"),
    sourceTab("netease", "网易云音乐"),
  );

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
    sourceTabs,
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

function sourceTab(source: Source, label: string): HTMLButtonElement {
  const button = h(
    "button",
    {
      class: "search__source",
      type: "button",
      "aria-selected": String(source === state?.source),
      "data-source": source,
    },
    label,
  ) as HTMLButtonElement;
  button.addEventListener("click", () => {
    if (!state || state.source === source) return;
    state.source = source;
    state.items = [];
    state.page = 0;
    state.hasMore = true;
    state.error = "";
    state.token += 1;
    renderResults();
    if (state.keyword.trim()) void loadNextPage();
  });
  return button;
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
    const result: SearchPage =
      current.source === "bili"
        ? await searchBili(keyword, page)
        : await searchNetease(keyword, page);
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
    "button",
    {
      class: "search-result",
      type: "button",
      "aria-label": `添加 ${item.title}`,
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
  ) as HTMLButtonElement;

  row.addEventListener("click", () => {
    if (!state || state.added.has(`${item.source}:${item.sourceId}`)) return;
    void addItem(item, row, action);
  });
  return row;
}

async function addItem(
  item: SearchResultItem,
  row: HTMLButtonElement,
  action: HTMLButtonElement,
): Promise<void> {
  if (!state) return;
  const key = `${item.source}:${item.sourceId}`;
  if (state.added.has(key)) return;
  row.disabled = true;
  action.textContent = "添加中";
  try {
    const created = await flowAdd(item);
    state.added.add(key);
    store.set({ flow: [...store.get().flow, created] });
    action.textContent = "已添加";
    action.disabled = true;
    showToast("已加入学习流");
  } catch (error) {
    row.disabled = false;
    action.textContent = "添加";
    showToast(error instanceof Error ? error.message : "添加失败", {
      error: true,
    });
  }
}
