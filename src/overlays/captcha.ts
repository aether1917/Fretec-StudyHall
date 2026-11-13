import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { onEvent } from "../api";
import { showToast } from "../ui/toast";
import type { CaptchaConfig } from "../types";

export interface CaptchaResult {
  challenge: string;
  validate: string;
  seccode: string;
}

let pending: Promise<CaptchaResult> | null = null;

export async function runBiliCaptcha(
  config: CaptchaConfig,
  title: string,
): Promise<CaptchaResult> {
  if (pending) return pending;
  const task = createCaptchaWindow(config, title);
  pending = task;
  try {
    return await task;
  } finally {
    pending = null;
  }
}

async function createCaptchaWindow(
  config: CaptchaConfig,
  title: string,
): Promise<CaptchaResult> {
  const label = `captcha-${Date.now()}`;
  const params = new URLSearchParams({
    gt: config.gt,
    challenge: config.challenge,
  });
  const win = new WebviewWindow(label, {
    url: `captcha.html?${params.toString()}`,
    title,
    width: 420,
    height: 560,
    resizable: false,
    center: true,
    alwaysOnTop: true,
    focus: true,
  });

  return new Promise<CaptchaResult>((resolve, reject) => {
    let settled = false;
    let unlisten: (() => void) | null = null;
    let timeout: number | null = null;

    const cleanup = () => {
      if (unlisten) unlisten();
      if (timeout !== null) window.clearTimeout(timeout);
      unlisten = null;
      timeout = null;
    };

    const finish = (result: CaptchaResult) => {
      if (settled) return;
      settled = true;
      cleanup();
      void win.close().catch(() => undefined);
      resolve(result);
    };

    const fail = (message: string) => {
      if (settled) return;
      settled = true;
      cleanup();
      void win.close().catch(() => undefined);
      showToast(message, { error: true, duration: 3200 });
      reject(new Error(message));
    };

    void onEvent<CaptchaResult>("captcha://result", (payload) => {
      finish(payload);
    }).then((off) => {
      unlisten = off;
    });

    void win.once("tauri://error", () => fail("无法打开人机验证窗口"));
    void win.once("tauri://destroyed", () => {
      if (!settled) fail("人机验证已取消");
    });

    timeout = window.setTimeout(
      () => fail("人机验证超时，请重试或改用二维码登录"),
      120000,
    );
  });
}
