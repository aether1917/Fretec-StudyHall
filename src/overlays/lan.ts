import { lanDisconnect, lanStart } from "../api";
import { h } from "../dom";
import { store } from "../store";
import type { LanPeer, LanStatus } from "../types";
import { icon } from "../ui/icons";
import { showToast } from "../ui/toast";
import { closeOverlay, showOverlay } from "./manager";

let bodyEl: HTMLDivElement | null = null;
let unsubscribe: (() => void) | null = null;

export function openLanOverlay(): void {
  const backBtn = h(
    "button",
    { class: "icon-btn", type: "button", "aria-label": "返回" },
    icon("back"),
  );
  backBtn.addEventListener("click", () => closeOverlay());

  const header = h(
    "div",
    { class: "overlay-header" },
    backBtn,
    h("div", { class: "overlay-header__title" }, "多设备互联"),
  );

  bodyEl = h("div", { class: "lan__body" }) as HTMLDivElement;
  const root = h("div", { class: "overlay-layer lan" }, header, bodyEl);

  const opened = showOverlay("lan", root, () => {
    unsubscribe?.();
    unsubscribe = null;
    bodyEl = null;
  });
  if (!opened) return;

  render(store.get().lan);
  unsubscribe = store.subscribe((state) => render(state.lan));
  void startSearch();
}

async function startSearch(): Promise<void> {
  try {
    const status = await lanStart();
    store.set({ lan: status });
  } catch (error) {
    showToast(error instanceof Error ? error.message : "无法开始搜索", {
      error: true,
    });
  }
}

function render(lan: LanStatus | null): void {
  if (!bodyEl) return;
  if (!lan) {
    bodyEl.replaceChildren(
      h("div", { class: "lan__empty" }, "正在初始化局域网…"),
    );
    return;
  }

  const selfName = h("div", { class: "lan__device-name" }, lan.self.name);
  const selfCard = h(
    "div",
    { class: "lan__self" },
    h("div", { class: "lan__label" }, "本机设备名称"),
    selfName,
    h(
      "div",
      { class: "lan__status-text" },
      lan.searching ? "正在搜索局域网设备…" : lan.message || "等待搜索",
    ),
  );

  const statusRow = h(
    "div",
    { class: "lan__status-row" },
    h(
      "div",
      { class: "lan__status-text" },
      lan.searching ? "搜索中…" : "搜索已结束",
    ),
    h("div", { class: "lan__spacer", style: "flex:1" }),
    (() => {
      const retry = h(
        "button",
        {
          class: "tonal-button",
          type: "button",
          disabled: !lan.canConnect ? "" : undefined,
        },
        "重新搜索",
      ) as HTMLButtonElement;
      retry.addEventListener("click", () => {
        void startSearch();
      });
      return retry;
    })(),
  );

  const chunks: Node[] = [selfCard];

  if (!lan.available) {
    chunks.push(
      h(
        "div",
        { class: "lan__unavailable" },
        "当前没有可用的局域网，多设备互联暂不可用。连接 Wi-Fi 或组网后点击「重新搜索」。",
      ),
    );
  } else if (!lan.canConnect) {
    chunks.push(
      h(
        "div",
        { class: "lan__unavailable" },
        "倒计时进行中，暂不可发起或接受新的设备连接。暂停或结束后再试。",
      ),
    );
  } else if (lan.error) {
    chunks.push(h("div", { class: "lan__unavailable" }, lan.error));
  }

  chunks.push(statusRow);

  chunks.push(
    h("div", { class: "lan__section-title" }, `已连接设备（${lan.peers.filter((p) => p.online).length}）`),
  );

  const online = lan.peers.filter((peer) => peer.online);
  if (online.length === 0) {
    chunks.push(
      h(
        "div",
        { class: "lan__empty" },
        lan.searching ? "正在发现设备…" : "未发现设备",
      ),
    );
  } else {
    chunks.push(
      h(
        "div",
        { class: "lan__list" },
        ...online.map((peer) => deviceRow(peer, lan)),
      ),
    );
  }

  bodyEl.replaceChildren(...chunks);
}

function deviceRow(peer: LanPeer, lan: LanStatus): HTMLElement {
  const disconnect = h(
    "button",
    {
      class: "text-button",
      type: "button",
      disabled: !lan.canConnect ? "" : undefined,
    },
    "断开",
  ) as HTMLButtonElement;
  disconnect.addEventListener("click", () => {
    void lanDisconnect(peer.id)
      .then(() => showToast(`已断开 ${peer.name}`))
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "断开失败", {
          error: true,
        }),
      );
  });

  return h(
    "div",
    { class: "lan__device" },
    h(
      "div",
      { class: "lan__device-main" },
      h("div", { class: "lan__device-name-row" }, peer.name),
      h(
        "div",
        { class: "lan__device-state" },
        h("span", {
          class: `lan__online-dot${peer.online ? "" : " lan__online-dot--offline"}`,
        }),
        peer.online ? "在线" : "已断开",
      ),
    ),
    disconnect,
  );
}
