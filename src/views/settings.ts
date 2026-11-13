import * as api from "../api";
import { h } from "../dom";
import { showToast } from "../ui/toast";
import { getThemeMode, setThemeMode, type ThemeMode } from "../theme";

const isAndroid = /Android/i.test(navigator.userAgent);
const REPO_URL = "https://gitee.com/aether2000/Fretec-StudyHall";
const RELEASES_URL = `${REPO_URL}/releases`;

export function renderSettingsPage(container: HTMLElement): void {
  container.replaceChildren();
  const root = h("div", { class: "settings" });
  root.append(
    buildDeviceSection(),
    buildAppearanceSection(),
    ...(isAndroid ? [] : [buildBehaviorSection()]),
    ...(isAndroid ? [buildPermissionSection()] : []),
    buildAboutSection(),
  );
  container.append(root);
}

/* ---------------- 设备名称 ---------------- */

function buildDeviceSection(): HTMLElement {
  const input = h("input", {
    class: "text-field",
    type: "text",
    maxlength: "40",
    "aria-label": "设备名称",
  }) as HTMLInputElement;
  const save = h(
    "button",
    { class: "filled-button", type: "button" },
    "保存",
  ) as HTMLButtonElement;

  void api
    .deviceGetName()
    .then((name) => {
      input.value = name;
    })
    .catch(() => undefined);

  const submit = () => {
    const name = input.value.trim();
    if (!name) {
      showToast("设备名不能为空");
      return;
    }
    save.disabled = true;
    void api
      .deviceSetName(name)
      .then(() => showToast("设备名已保存"))
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "保存失败", {
          error: true,
        }),
      )
      .finally(() => {
        save.disabled = false;
      });
  };
  save.addEventListener("click", submit);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") submit();
  });

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "设备"),
    h(
      "div",
      { class: "settings-card" },
      h("div", { class: "settings-card__field-row" }, input, save),
      h(
        "div",
        { class: "settings-card__hint" },
        "名称用于局域网内识别本机，修改后会同步给已连接设备。",
      ),
    ),
  );
}

/* ---------------- 外观 ---------------- */

function segmented(
  options: Array<{ value: string; label: string }>,
  selected: string,
  onSelect: (value: string) => void,
): { element: HTMLElement; mark: (value: string) => void } {
  const element = h("div", { class: "settings-segmented", role: "tablist" });
  const buttons: HTMLButtonElement[] = [];
  const mark = (value: string) => {
    for (const button of buttons) {
      button.setAttribute("aria-selected", String(button.dataset.value === value));
    }
  };
  for (const option of options) {
    const button = h(
      "button",
      {
        class: "settings-segmented__item",
        type: "button",
        role: "tab",
        "data-value": option.value,
        "aria-selected": String(option.value === selected),
      },
      option.label,
    ) as HTMLButtonElement;
    button.addEventListener("click", () => {
      mark(option.value);
      onSelect(option.value);
    });
    buttons.push(button);
    element.append(button);
  }
  return { element, mark };
}

function buildAppearanceSection(): HTMLElement {
  const current = getThemeMode();
  const { element, mark } = segmented(
    [
      { value: "light", label: "浅色" },
      { value: "dark", label: "深色" },
      { value: "system", label: "跟随系统" },
    ],
    current,
    (value) => {
      const mode = value as ThemeMode;
      setThemeMode(mode);
      void api.settingsSet({ theme: mode }).catch(() => undefined);
    },
  );
  void api
    .settingsGet()
    .then((settings) => {
      if (
        settings.theme === "light" ||
        settings.theme === "dark" ||
        settings.theme === "system"
      ) {
        mark(settings.theme);
      }
    })
    .catch(() => undefined);

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "外观"),
    h(
      "div",
      { class: "settings-card" },
      h(
        "div",
        { class: "settings-card__row" },
        h("div", { class: "settings-card__heading" }, "显示模式"),
      ),
      h(
        "div",
        { class: "settings-card__hint" },
        "默认浅色，也可以跟随系统自动切换。",
      ),
      element,
    ),
  );
}

/* ---------------- 关闭窗口行为（桌面） ---------------- */

function buildBehaviorSection(): HTMLElement {
  const { element, mark } = segmented(
    [
      { value: "hide", label: "隐藏到托盘" },
      { value: "quit", label: "直接退出" },
    ],
    "hide",
    (value) => {
      void api
        .settingsSet({ closeBehavior: value })
        .then(() => showToast("已保存，下次关闭窗口时生效"))
        .catch((error: unknown) =>
          showToast(error instanceof Error ? error.message : "保存失败", {
            error: true,
          }),
        );
    },
  );
  void api
    .settingsGet()
    .then((settings) => {
      if (settings.closeBehavior === "hide" || settings.closeBehavior === "quit") {
        mark(settings.closeBehavior);
      }
    })
    .catch(() => undefined);

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "窗口"),
    h(
      "div",
      { class: "settings-card" },
      h(
        "div",
        { class: "settings-card__row" },
        h("div", { class: "settings-card__heading" }, "关闭按钮"),
      ),
      h(
        "div",
        { class: "settings-card__hint" },
        "隐藏到托盘时计时和局域网同步继续运行；直接退出会结束进程。计时进行中关闭按钮不会退出，先结束计时。",
      ),
      element,
    ),
  );
}

/* ---------------- 权限（Android） ---------------- */

function buildPermissionSection(): HTMLElement {
  const status = h(
    "div",
    { class: "settings-card__hint" },
    "正在检查通知权限…",
  );
  const refresh = () => {
    void api
      .nativeNotificationStatus()
      .then((granted) => {
        status.textContent = granted ? "通知权限已开启" : "通知权限未开启";
      })
      .catch(() => {
        status.textContent = "无法读取通知权限状态";
      });
  };
  refresh();

  const request = h(
    "button",
    { class: "tonal-button", type: "button" },
    "申请通知权限",
  ) as HTMLButtonElement;
  request.addEventListener("click", () => {
    request.disabled = true;
    void api
      .nativeNotificationRequest()
      .then(() => {
        showToast("已请求通知权限");
        window.setTimeout(refresh, 800);
      })
      .catch((error: unknown) =>
        showToast(error instanceof Error ? error.message : "请求失败", {
          error: true,
        }),
      )
      .finally(() => {
        window.setTimeout(() => {
          request.disabled = false;
        }, 900);
      });
  });

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "权限"),
    h(
      "div",
      { class: "settings-card" },
      h(
        "div",
        { class: "settings-card__row" },
        h("div", { class: "settings-card__heading" }, "通知"),
      ),
      status,
      h("div", { class: "settings-card__actions" }, request),
      h(
        "div",
        { class: "settings-card__hint" },
        "计时开始时还会申请全屏和屏幕固定，用来锁住当前应用；结束后自动恢复。",
      ),
    ),
  );
}

/* ---------------- 关于与更新 ---------------- */

function copyText(text: string): void {
  const clipboard = navigator.clipboard;
  if (!clipboard) {
    showToast("无法访问剪贴板，请手动选择链接");
    return;
  }
  void clipboard
    .writeText(text)
    .then(() => showToast("链接已复制"))
    .catch(() => showToast("复制失败，请手动选择链接", { error: true }));
}

function linkRow(label: string, url: string): HTMLElement {
  const copy = h(
    "button",
    { class: "text-button", type: "button" },
    "复制链接",
  ) as HTMLButtonElement;
  copy.addEventListener("click", () => copyText(url));
  return h(
    "div",
    { class: "settings-card__field-row" },
    h("div", { class: "settings-card__hint settings-link" }, `${label}：${url}`),
    copy,
  );
}

function buildAboutSection(): HTMLElement {
  const version = h("div", { class: "settings-card__hint" }, "版本读取中…");
  void api
    .appVersion()
    .then((value) => {
      version.textContent = `当前版本：${value}`;
    })
    .catch(() => {
      version.textContent = "当前版本：未知";
    });

  const result = h("div", { class: "settings-card__hint" });
  const check = h(
    "button",
    { class: "tonal-button", type: "button" },
    "检查更新",
  ) as HTMLButtonElement;
  check.addEventListener("click", () => {
    check.disabled = true;
    result.textContent = "正在检查…";
    void api
      .checkUpdate()
      .then((info) => {
        result.replaceChildren();
        if (info.hasUpdate) {
          result.append(
            h("div", {}, `发现新版本 ${info.latest}（当前 ${info.current}）`),
            h("div", { class: "settings-link" }, info.url),
          );
          const copy = h(
            "button",
            { class: "text-button", type: "button" },
            "复制下载链接",
          ) as HTMLButtonElement;
          copy.addEventListener("click", () => copyText(info.url));
          result.append(copy);
        } else {
          result.textContent = `已是最新版本（${info.current}）`;
        }
      })
      .catch((error: unknown) => {
        result.textContent =
          error instanceof Error ? error.message : "检查更新失败";
      })
      .finally(() => {
        check.disabled = false;
      });
  });

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "关于"),
    h(
      "div",
      { class: "settings-card" },
      h(
        "div",
        { class: "settings-card__row" },
        h("div", { class: "settings-card__heading" }, "Fretec StudyHall"),
      ),
      version,
      h("div", { class: "settings-card__actions" }, check),
      result,
      linkRow("项目主页", REPO_URL),
      linkRow("发行版", RELEASES_URL),
    ),
  );
}
