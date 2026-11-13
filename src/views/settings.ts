import QRCode from "qrcode";
import * as api from "../api";
import { h } from "../dom";
import { store } from "../store";
import type {
  AccountStatus,
  AuthStatus,
  QrLogin,
  QrPoll,
  Source,
} from "../types";
import { icon } from "../ui/icons";
import { showToast } from "../ui/toast";
import { runBiliCaptcha, type CaptchaResult } from "../overlays/captcha";

const isAndroid = /Android/i.test(navigator.userAgent);

let unsubscribe: (() => void) | null = null;
let accountsHost: HTMLElement | null = null;
let activeDialogClose: (() => void) | null = null;

export function renderSettingsPage(container: HTMLElement): void {
  cleanupPage();
  container.replaceChildren();

  const root = h("div", { class: "settings" });
  root.append(buildDeviceSection(), buildAccountsSection());
  container.append(root);

  let lastAuth: AuthStatus = store.get().auth;
  unsubscribe = store.subscribe((state) => {
    if (state.auth !== lastAuth) {
      lastAuth = state.auth;
      refreshAccounts();
    }
    if (state.highlight !== null) {
      window.requestAnimationFrame(() => {
        document
          .querySelector(`.settings-card[data-account="${state.highlight}"]`)
          ?.scrollIntoView({ block: "nearest" });
      });
    }
  });
}

function cleanupPage(): void {
  unsubscribe?.();
  unsubscribe = null;
  activeDialogClose?.();
  activeDialogClose = null;
  accountsHost = null;
}

/* ---------------- 设备名称 ---------------- */

function buildDeviceSection(): HTMLElement {
  const input = h("input", {
    class: "text-field",
    type: "text",
    maxlength: "40",
    placeholder: "设备名称",
    "aria-label": "设备名称",
  }) as HTMLInputElement;

  const save = h(
    "button",
    { class: "filled-button", type: "button" },
    "保存",
  ) as HTMLButtonElement;

  const saveName = async () => {
    const name = input.value.trim();
    if (!name) {
      showToast("设备名称不能为空", { error: true });
      return;
    }
    save.disabled = true;
    try {
      const saved = await api.deviceSetName(name);
      input.value = saved;
      const lan = store.get().lan;
      if (lan) {
        store.set({
          lan: { ...lan, self: { ...lan.self, name: saved } },
        });
      }
      showToast("设备名称已保存");
    } catch (error) {
      showToast(error instanceof Error ? error.message : "保存失败", {
        error: true,
      });
    } finally {
      save.disabled = false;
    }
  };
  save.addEventListener("click", () => void saveName());
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") void saveName();
  });

  void api
    .deviceGetName()
    .then((name) => {
      input.value = name;
    })
    .catch(() => undefined);

  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "设备"),
    h(
      "div",
      { class: "settings-card" },
      h(
        "div",
        { class: "settings-card__field-row" },
        input,
        save,
      ),
      h(
        "div",
        { class: "settings-card__hint" },
        "名称用于局域网内识别本机，修改后会同步给已连接设备。",
      ),
    ),
  );
}

/* ---------------- 账号 ---------------- */

function buildAccountsSection(): HTMLElement {
  accountsHost = h("div", { class: "settings__group" });
  refreshAccounts();
  return h(
    "div",
    { class: "settings__group" },
    h("div", { class: "settings__group-title" }, "账号"),
    accountsHost,
  );
}

function refreshAccounts(): void {
  if (!accountsHost) return;
  accountsHost.replaceChildren(
    buildAccountCard("bili"),
    buildAccountCard("netease"),
  );
}

function accountOf(source: Source): AccountStatus {
  return store.get().auth[source];
}

function buildAccountCard(source: Source): HTMLElement {
  const account = accountOf(source);
  const label = source === "bili" ? "哔哩哔哩" : "网易云音乐";
  const highlighted = store.get().highlight === source;

  const card = h("div", {
    class: `settings-card${highlighted ? " is-highlighted" : ""}`,
    "data-account": source,
  }) as HTMLElement;

  card.append(
    h(
      "div",
      { class: "settings-card__row" },
      h("div", { class: "settings-card__heading" }, label),
      h(
        "div",
        { class: "settings-card__hint" },
        account.loggedIn ? "已登录" : "未登录",
      ),
    ),
  );

  if (account.loggedIn) {
    const avatar = h("img", {
      class: "settings-card__avatar",
      src: account.avatar,
      alt: "",
    }) as HTMLImageElement;
    avatar.addEventListener("error", () => {
      avatar.replaceWith(h("div", { class: "settings-card__avatar" }));
    });
    const logout = h(
      "button",
      { class: "text-button", type: "button" },
      "退出登录",
    ) as HTMLButtonElement;
    logout.addEventListener("click", () => {
      logout.disabled = true;
      void api
        .authLogout(source)
        .then((auth) => {
          store.set({ auth });
          showToast("已退出登录");
        })
        .catch((error: unknown) => {
          logout.disabled = false;
          showToast(error instanceof Error ? error.message : "退出失败", {
            error: true,
          });
        });
    });
    card.append(
      h(
        "div",
        { class: "settings-card__profile" },
        avatar,
        h(
          "div",
          { class: "settings-card__profile-name" },
          account.name || label,
        ),
        logout,
      ),
    );
  } else {
    const qrBtn = h(
      "button",
      { class: "tonal-button", type: "button" },
      icon("qr"),
      "二维码登录",
    ) as HTMLButtonElement;
    qrBtn.addEventListener("click", () => openQrDialog(source));

    const phoneBtn = h(
      "button",
      { class: "filled-button", type: "button" },
      icon("phone"),
      "手机号登录",
    ) as HTMLButtonElement;
    phoneBtn.addEventListener("click", () => openPhoneSheet(source));

    card.append(
      h(
        "div",
        { class: "settings-card__actions" },
        qrBtn,
        phoneBtn,
      ),
    );
  }

  return card;
}

/* ---------------- 二维码登录 ---------------- */

function openQrDialog(source: Source): void {
  activeDialogClose?.();
  const panel = h("div", { class: "dialog__panel" });
  const scrim = h("div", { class: "dialog" }, panel);
  const img = h("img", { class: "dialog__qr", alt: "登录二维码" });
  const status = h("div", { class: "dialog__status" }, "正在生成二维码…");
  const regen = h(
    "button",
    { class: "tonal-button", type: "button" },
    "重新生成",
  ) as HTMLButtonElement;
  regen.hidden = true;
  const close = h(
    "button",
    { class: "text-button", type: "button" },
    "关闭",
  ) as HTMLButtonElement;

  panel.append(img, status, h("div", { class: "dialog__actions" }, regen, close));
  document.body.append(scrim);

  let timer: number | null = null;
  let key = "";
  let polling = false;
  let closed = false;

  const stopTimer = () => {
    if (timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  };

  const closeDialog = () => {
    if (closed) return;
    closed = true;
    stopTimer();
    scrim.remove();
    if (activeDialogClose === closeDialog) activeDialogClose = null;
  };
  activeDialogClose = closeDialog;

  const onPoll = async (result: QrPoll) => {
    status.textContent = result.message;
    if (result.state === "success") {
      stopTimer();
      if (result.profile) {
        const current = store.get().auth;
        store.set({
          auth: { ...current, [source]: { loggedIn: true, ...result.profile } },
        });
      }
      const auth = await api.authStatus().catch(() => store.get().auth);
      store.set({ auth });
      showToast("登录成功");
      closeDialog();
    } else if (result.state === "expired" || result.state === "error") {
      stopTimer();
      regen.hidden = false;
    }
  };

  const pollOnce = async () => {
    if (closed || polling || !key) return;
    polling = true;
    try {
      onPoll(
        source === "bili"
          ? await api.biliQrPoll(key)
          : await api.neteaseQrPoll(key),
      );
    } catch (error) {
      stopTimer();
      regen.hidden = false;
      status.textContent =
        error instanceof Error ? error.message : "二维码状态检查失败";
    } finally {
      polling = false;
    }
  };

  const generate = async () => {
    regen.hidden = true;
    img.hidden = true;
    status.textContent = "正在生成二维码…";
    try {
      const login: QrLogin =
        source === "bili"
          ? await api.biliQrGenerate()
          : await api.neteaseQrGenerate();
      key = login.key;
      const dark = cssColor("--md-sys-color-inverse-on-surface");
      const light = cssColor("--md-sys-color-inverse-surface");
      img.src = await QRCode.toDataURL(login.url, {
        margin: 1,
        width: 440,
        color: { dark, light },
      });
      img.hidden = false;
      status.textContent = "打开对应 App 扫码登录";
      stopTimer();
      timer = window.setInterval(() => {
        void pollOnce();
      }, 2000);
    } catch (error) {
      status.textContent =
        error instanceof Error ? error.message : "二维码生成失败";
      regen.hidden = false;
    }
  };

  close.addEventListener("click", closeDialog);
  regen.addEventListener("click", () => void generate());
  scrim.addEventListener("click", (event) => {
    if (event.target === scrim) closeDialog();
  });
  void generate();
}

function cssColor(variable: string): string {
  const root = getComputedStyle(document.documentElement);
  const value = root.getPropertyValue(variable).trim();
  if (value) return value;
  return root.getPropertyValue("--md-sys-color-on-surface").trim();
}

/* ---------------- 手机号登录弹层 ---------------- */

function openPhoneSheet(source: Source): void {
  activeDialogClose?.();
  const label = source === "bili" ? "哔哩哔哩" : "网易云音乐";
  let mode: "code" | "password" = "code";
  let countdown = 0;
  let countdownTimer: number | null = null;
  let captchaKey = "";
  let captchaPhone = "";

  const scrim = h("div", { class: "sheet-scrim is-open" });
  const sheet = h("div", { class: "sheet" });
  const errorEl = h("div", { class: "sheet__error" });
  const phoneInput = h("input", {
    class: "text-field",
    type: "tel",
    placeholder: "手机号",
    "aria-label": "手机号",
    inputmode: "tel",
    autocomplete: "tel",
  }) as HTMLInputElement;
  const codeInput = h("input", {
    class: "text-field",
    type: "text",
    placeholder: "6 位验证码",
    "aria-label": "验证码",
    inputmode: "numeric",
    maxlength: "6",
  }) as HTMLInputElement;
  const passwordInput = h("input", {
    class: "text-field",
    type: "password",
    placeholder: "密码",
    "aria-label": "密码",
  }) as HTMLInputElement;
  const sendCodeBtn = h(
    "button",
    { class: "tonal-button sheet__code-btn", type: "button" },
    "获取验证码",
  ) as HTMLButtonElement;
  const submit = h(
    "button",
    { class: "filled-button", type: "button" },
    "登录",
  ) as HTMLButtonElement;

  const codeRow = h(
    "div",
    { class: "sheet__inline" },
    codeInput,
    sendCodeBtn,
  );
  const cancel = h(
    "button",
    { class: "text-button", type: "button" },
    "取消",
  ) as HTMLButtonElement;
  const form = h(
    "div",
    { class: "sheet__form" },
    phoneInput,
    passwordInput,
    codeRow,
    errorEl,
    submit,
    cancel,
  );
  passwordInput.style.display = "none";

  const tabs = h(
    "div",
    { class: "sheet__tabs" },
    tabButton("验证码", "code"),
    ...(isAndroid ? [tabButton("密码", "password")] : []),
  );

  const close = () => {
    if (countdownTimer !== null) window.clearInterval(countdownTimer);
    countdownTimer = null;
    scrim.remove();
    sheet.remove();
    if (activeDialogClose === close) activeDialogClose = null;
  };
  activeDialogClose = close;

  function tabButton(text: string, value: "code" | "password"): HTMLButtonElement {
    const button = h(
      "button",
      {
        class: "sheet__tab",
        type: "button",
        "data-mode": value,
        "aria-selected": String(value === mode),
      },
      text,
    ) as HTMLButtonElement;
    button.addEventListener("click", () => {
      mode = value;
      for (const tab of Array.from(sheet.querySelectorAll(".sheet__tab"))) {
        tab.setAttribute(
          "aria-selected",
          String((tab as HTMLElement).dataset.mode === value),
        );
      }
      codeRow.style.display = value === "code" ? "flex" : "none";
      passwordInput.style.display = value === "password" ? "" : "none";
      submit.textContent = "登录";
    });
    return button;
  }

  sheet.append(
    h("div", { class: "sheet__handle" }),
    h("div", { class: "sheet__title" }, `${label}手机号登录`),
    tabs,
    form,
  );

  cancel.addEventListener("click", close);
  scrim.addEventListener("click", close);

  const setError = (message: string) => {
    errorEl.textContent = message;
  };

  const startCountdown = () => {
    countdown = 60;
    sendCodeBtn.disabled = true;
    sendCodeBtn.textContent = "60s 后重发";
    countdownTimer = window.setInterval(() => {
      countdown -= 1;
      if (countdown <= 0) {
        if (countdownTimer !== null) window.clearInterval(countdownTimer);
        countdownTimer = null;
        sendCodeBtn.disabled = false;
        sendCodeBtn.textContent = "获取验证码";
        return;
      }
      sendCodeBtn.textContent = `${countdown}s 后重发`;
    }, 1000);
  };

  sendCodeBtn.addEventListener("click", async () => {
    const phone = phoneInput.value.trim();
    if (!/^\d{6,15}$/.test(phone)) {
      setError("请输入正确的手机号");
      return;
    }
    setError("");
    sendCodeBtn.disabled = true;
    sendCodeBtn.textContent = "发送中…";
    try {
      if (source === "bili") {
        const config = await api.biliCaptchaConfig();
        const result: CaptchaResult = await runBiliCaptcha(config, "人机验证");
        const ticket = await api.biliSmsSend(
          phone,
          "86",
          result.challenge,
          result.validate,
          result.seccode,
        );
        captchaKey = ticket.captchaKey;
        captchaPhone = phone;
      } else {
        await api.neteaseSmsSend(phone);
        captchaKey = "";
        captchaPhone = phone;
      }
      showToast("验证码已发送");
      startCountdown();
    } catch (error) {
      sendCodeBtn.disabled = false;
      sendCodeBtn.textContent = "获取验证码";
      setError(error instanceof Error ? error.message : "验证码发送失败");
    }
  });

  submit.addEventListener("click", async () => {
    const phone = phoneInput.value.trim();
    if (!/^\d{6,15}$/.test(phone)) {
      setError("请输入正确的手机号");
      return;
    }
    setError("");
    submit.disabled = true;
    submit.textContent = "登录中…";
    try {
      if (mode === "password") {
        if (!passwordInput.value) {
          setError("请输入密码");
          return;
        }
        if (source === "bili") {
          try {
            await api.biliLoginPassword(phone, passwordInput.value, null);
          } catch (error) {
            const message =
              error instanceof Error ? error.message : String(error);
            if (!message.includes("NEED_CAPTCHA")) throw error;
            const config = await api.biliCaptchaConfig();
            const captcha = await runBiliCaptcha(config, "人机验证");
            await api.biliLoginPassword(
              phone,
              passwordInput.value,
              `${captcha.challenge}|${captcha.validate}|${captcha.seccode}`,
            );
          }
        } else {
          await api.neteaseLoginPassword(phone, passwordInput.value);
        }
      } else if (source === "bili") {
        const code = codeInput.value.trim();
        if (code.length < 4) {
          setError("请输入验证码");
          return;
        }
        if (captchaPhone !== phone || !captchaKey) {
          setError("请先获取验证码");
          return;
        }
        await api.biliSmsLogin(phone, "86", code, captchaKey);
      } else {
        const code = codeInput.value.trim();
        if (code.length < 4) {
          setError("请输入验证码");
          return;
        }
        await api.neteaseSmsLogin(phone, code);
      }
      const auth = await api.authStatus();
      store.set({ auth });
      showToast("登录成功");
      close();
    } catch (error) {
      setError(error instanceof Error ? error.message : "登录失败");
    } finally {
      submit.disabled = false;
      submit.textContent = "登录";
    }
  });

  document.body.append(scrim, sheet);
  window.requestAnimationFrame(() => sheet.classList.add("is-open"));
  phoneInput.focus();
}
