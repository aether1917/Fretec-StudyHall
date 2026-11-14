import { flowRemove, flowReorder } from "../api";
import { h } from "../dom";
import { store } from "../store";
import type { FlowItem } from "../types";
import { showToast } from "../ui/toast";
import { icon } from "../ui/icons";
import { openPlayer } from "../overlays/player";

const SWIPE_LIMIT_RATIO = 0.3;

let openRow: { row: HTMLElement; card: HTMLElement } | null = null;

function translateX(el: HTMLElement): number {
  const transform = getComputedStyle(el).transform;
  if (!transform || transform === "none") return 0;
  const match = /matrix\(([^)]+)\)/.exec(transform);
  if (!match) return 0;
  const parts = match[1]?.split(",").map((value) => Number(value.trim()));
  return parts && parts.length >= 6 ? (parts[4] ?? 0) : 0;
}

export function renderFlowPage(container: HTMLElement): void {
  openRow = null;
  const items = store.get().flow;
  container.replaceChildren();

  if (items.length === 0) {
    const emptyIcon = icon("flow");
    emptyIcon.classList.add("flow-empty__icon");
    container.append(
      h(
        "div",
        { class: "flow-empty" },
        emptyIcon,
        h("div", { class: "flow-empty__title" }, "学习流还是空的"),
        h(
          "div",
          { class: "flow-empty__text" },
          "点击左上角搜索图标，把哔哩哔哩视频加入这里。",
        ),
      ),
    );
    return;
  }

  const list = h("div", { class: "flow", id: "flow-list" });
  items.forEach((item, index) => {
    list.append(buildRow(item, index, list));
  });
  container.append(list);
}

function closeOpenRow(): void {
  if (!openRow) return;
  const { row, card } = openRow;
  openRow = null;
  card.style.transform = "translateX(0)";
  row.removeAttribute("data-swiped");
}

function openRowAt(row: HTMLElement, card: HTMLElement, width: number): void {
  if (openRow && openRow.row !== row) closeOpenRow();
  const offset = Math.max(96, width * SWIPE_LIMIT_RATIO);
  card.style.transform = `translateX(${-offset}px)`;
  row.dataset.swiped = "true";
  openRow = { row, card };
}

function buildRow(
  item: FlowItem,
  index: number,
  list: HTMLElement,
): HTMLElement {
  const cover = h("img", {
    alt: "",
    loading: "lazy",
    referrerpolicy: "no-referrer",
  }) as HTMLImageElement;
  const fallback = h(
    "div",
    { class: "flow-card__cover-fallback" },
    icon(item.source === "bili" ? "video" : "music"),
  );
  if (item.cover) {
    cover.src = item.cover;
    cover.addEventListener("error", () => {
      cover.style.display = "none";
      fallback.style.display = "grid";
    });
    fallback.style.display = "none";
  } else {
    cover.style.display = "none";
  }

  const progress = Math.max(
    0,
    Math.min(1, item.durationSec > 0 ? item.progress.positionSec / item.durationSec : 0),
  );

  const title = h("div", { class: "flow-card__title" }, item.title);
  const meta = h(
    "div",
    { class: "flow-card__meta" },
    h(
      "span",
      { class: "flow-card__source" },
      item.source === "bili" ? "B站" : "已移除",
    ),
    " ",
    item.author || "未知作者",
  );

  const handle = h(
    "div",
    { class: "flow-card__handle", title: "长按拖动排序" },
    icon("drag"),
  );

  const card = h(
    "div",
    { class: "flow-card", "data-index": String(index) },
    h("div", { class: "flow-card__cover" }, cover, fallback),
    h("div", { class: "flow-card__body" }, title, meta),
    handle,
    h(
      "div",
      { class: "flow-card__progress" },
      h("span", { style: `width:${(progress * 100).toFixed(2)}%` }),
    ),
  );

  const deleteBtn = h(
    "button",
    { class: "flow-row__delete", type: "button" },
    "删除",
  );
  deleteBtn.addEventListener("click", async (event) => {
    event.stopPropagation();
    deleteBtn.disabled = true;
    try {
      await flowRemove(item.id);
      const next = store.get().flow.filter((entry) => entry.id !== item.id);
      store.set({ flow: next });
      showToast("已删除");
    } catch (error) {
      deleteBtn.disabled = false;
      showToast(error instanceof Error ? error.message : "删除失败", {
        error: true,
      });
    }
  });

  const row = h(
    "div",
    { class: "flow-row", "data-id": item.id },
    deleteBtn,
    card,
  );

  attachInteractions(row, card, handle, item, list);
  return row;
}

function suppressNextClick(card: HTMLElement): void {
  const swallow = (event: MouseEvent) => {
    event.stopPropagation();
    event.preventDefault();
  };
  card.addEventListener("click", swallow, { capture: true, once: true });
  window.setTimeout(() => {
    card.removeEventListener("click", swallow, { capture: true } as never);
  }, 250);
}

function attachInteractions(
  row: HTMLElement,
  card: HTMLElement,
  handle: HTMLElement,
  item: FlowItem,
  list: HTMLElement,
): void {
  let pointerId: number | null = null;
  let startX = 0;
  let startY = 0;
  let startOffset = 0;
  let axis: "none" | "x" | "y" = "none";
  let moved = false;
  let swipeWidth = 0;

  const resetCard = () => {
    card.style.transform = "translateX(0)";
    row.removeAttribute("data-swiped");
    if (openRow?.row === row) openRow = null;
  };

  card.addEventListener("pointerdown", (event) => {
    if (event.button !== undefined && event.button !== 0) return;
    if ((event.target as HTMLElement).closest(".flow-card__handle")) return;
    if ((event.target as HTMLElement).closest(".flow-row__delete")) return;
    pointerId = event.pointerId;
    startX = event.clientX;
    startY = event.clientY;
    moved = false;
    axis = "none";
    swipeWidth = row.clientWidth;
    startOffset = row.dataset.swiped === "true" ? -(Math.max(96, swipeWidth * SWIPE_LIMIT_RATIO)) : 0;
    if (row.dataset.swiped === "true" && openRow?.row !== row) {
      // 其它已滑开的卡片先复位
      closeOpenRow();
      startOffset = 0;
    }
  });

  card.addEventListener("pointermove", (event) => {
    if (pointerId === null || event.pointerId !== pointerId) return;
    const dx = event.clientX - startX;
    const dy = event.clientY - startY;
    if (axis === "none") {
      if (Math.abs(dx) < 6 && Math.abs(dy) < 6) return;
      axis = Math.abs(dx) > Math.abs(dy) * 1.2 ? "x" : "y";
      if (axis === "x") {
        card.setPointerCapture?.(pointerId);
        card.style.transition = "none";
        if (openRow && openRow.row !== row) closeOpenRow();
      }
    }
    if (axis !== "x") return;
    moved = true;
    const maxOffset = Math.max(96, swipeWidth * SWIPE_LIMIT_RATIO);
    const offset = Math.min(0, Math.max(-maxOffset, startOffset + dx));
    card.style.transform = `translateX(${offset}px)`;
  });

  const finish = (event: PointerEvent) => {
    if (pointerId === null || event.pointerId !== pointerId) return;
    pointerId = null;
    card.style.transition = "";
    if (axis === "x") {
      const current = translateX(card);
      const threshold = swipeWidth * SWIPE_LIMIT_RATIO;
      if (-current >= threshold) {
        openRowAt(row, card, swipeWidth);
      } else {
        resetCard();
      }
      if (moved) suppressNextClick(card);
    }
    axis = "none";
  };

  card.addEventListener("pointerup", finish);
  card.addEventListener("pointercancel", (event) => {
    if (pointerId !== null && event.pointerId === pointerId) {
      pointerId = null;
      resetCard();
      axis = "none";
    }
  });

  card.addEventListener("click", (event) => {
    if (moved || row.dataset.dragged === "true") {
      event.stopPropagation();
      return;
    }
    if (row.dataset.swiped === "true") {
      resetCard();
      return;
    }
    if (openRow) closeOpenRow();
    void openItem(item.id);
  });

  /* ---------- 拖拽排序（手柄或长按卡片） ---------- */
  let dragState: {
    pointerId: number;
    startY: number;
    row: HTMLElement;
    placeholder: HTMLElement;
    rect: DOMRect;
  } | null = null;

  const beginDrag = (event: PointerEvent) => {
    if (dragState) return;
    const rect = row.getBoundingClientRect();
    const placeholder = document.createElement("div");
    placeholder.className = "flow-placeholder";
    placeholder.style.height = `${rect.height}px`;
    list.insertBefore(placeholder, row);
    row.classList.add("is-dragging");
    row.style.position = "fixed";
    row.style.left = `${rect.left}px`;
    row.style.top = `${rect.top}px`;
    row.style.width = `${rect.width}px`;
    row.style.zIndex = "60";
    row.style.pointerEvents = "none";
    dragState = {
      pointerId: event.pointerId,
      startY: event.clientY,
      row,
      placeholder,
      rect,
    };
    document.body.classList.add("flow-dragging");

    const move = (ev: PointerEvent) => {
      if (!dragState || ev.pointerId !== dragState.pointerId) return;
      ev.preventDefault();
      const dy = ev.clientY - dragState.startY;
      dragState.row.style.top = `${dragState.rect.top + dy}px`;
      const pointerY = ev.clientY;
      const children = Array.from(list.children).filter(
        (node) => node !== dragState?.placeholder,
      ) as HTMLElement[];
      let inserted = false;
      for (const child of children) {
        const box = child.getBoundingClientRect();
        if (pointerY < box.top + box.height / 2) {
          list.insertBefore(dragState.placeholder, child);
          inserted = true;
          break;
        }
      }
      if (!inserted) list.append(dragState.placeholder);
    };

    const end = (ev: PointerEvent) => {
      if (!dragState || ev.pointerId !== dragState.pointerId) return;
      const { row: dragged, placeholder } = dragState;
      placeholder.replaceWith(dragged);
      dragged.classList.remove("is-dragging");
      dragged.style.cssText = "";
      dragged.dataset.dragged = "true";
      window.setTimeout(() => {
        delete dragged.dataset.dragged;
      }, 260);
      document.body.classList.remove("flow-dragging");
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      dragState = null;
      suppressNextClick(card);

      const ids = Array.from(list.children)
        .map((node) => (node as HTMLElement).dataset.id)
        .filter((id): id is string => Boolean(id));
      const current = store.get().flow;
      if (ids.length === current.length && ids.join(",") !== current.map((entry) => entry.id).join(",")) {
        void persistOrder(ids);
      }
    };

    window.addEventListener("pointermove", move, { passive: false });
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };

  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== undefined && event.button !== 0) return;
    event.preventDefault();
    beginDrag(event);
  });

  let pressTimer: number | null = null;
  card.addEventListener("pointerdown", (event) => {
    if ((event.target as HTMLElement).closest(".flow-card__handle")) return;
    if (pressTimer !== null) window.clearTimeout(pressTimer);
    pressTimer = window.setTimeout(() => {
      pressTimer = null;
      if (moved) return;
      beginDrag(event);
    }, 420);
  });

  const cancelPress = () => {
    if (pressTimer !== null) {
      window.clearTimeout(pressTimer);
      pressTimer = null;
    }
  };
  card.addEventListener("pointerup", cancelPress);
  card.addEventListener("pointercancel", cancelPress);
  card.addEventListener("pointermove", (event) => {
    if (pressTimer !== null) {
      const dx = event.clientX - startX;
      const dy = event.clientY - startY;
      if (Math.abs(dx) > 8 || Math.abs(dy) > 8) cancelPress();
    }
  });
}

async function persistOrder(ids: string[]): Promise<void> {
  try {
    const updated = await flowReorder(ids);
    store.set({ flow: updated });
  } catch (error) {
    showToast(error instanceof Error ? error.message : "排序保存失败", {
      error: true,
    });
  }
}

async function openItem(itemId: string): Promise<void> {
  await openPlayer(itemId);
}
