import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface GeetestValidate {
  geetest_challenge: string;
  geetest_validate: string;
  geetest_seccode: string;
}

interface GeetestInstance {
  onSuccess(callback: () => void): void;
  onError(callback: (error: unknown) => void): void;
  onClose(callback: () => void): void;
  getValidate(): GeetestValidate;
}

declare global {
  interface Window {
    initGeetest?: (
      options: Record<string, unknown>,
      callback: (captcha: GeetestInstance) => void,
    ) => void;
  }
}

const statusEl = document.getElementById("status");

function setStatus(message: string): void {
  if (statusEl) statusEl.textContent = message;
}

function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error("验证组件加载失败"));
    document.head.append(script);
  });
}

async function main(): Promise<void> {
  const params = new URLSearchParams(location.search);
  const gt = params.get("gt") ?? "";
  const challenge = params.get("challenge") ?? "";
  if (!gt || !challenge) {
    setStatus("缺少验证参数，请返回重试");
    return;
  }

  try {
    await loadScript("https://static.geetest.com/static/js/gt.0.5.1.js");
  } catch {
    setStatus("人机验证组件加载失败，请检查网络后改用二维码登录");
    return;
  }

  if (!window.initGeetest) {
    setStatus("人机验证组件不可用，请改用二维码登录");
    return;
  }

  setStatus("请完成人机验证");
  window.initGeetest(
    {
      gt,
      challenge,
      offline: false,
      new_captcha: true,
      product: "bind",
      width: "100%",
      lang: "zh-cn",
    },
    (captcha) => {
      captcha.onSuccess(() => {
        const validate = captcha.getValidate();
        void invoke("bili_captcha_result", {
          challenge: validate.geetest_challenge,
          validate: validate.geetest_validate,
          seccode: validate.geetest_seccode,
        })
          .then(() => getCurrentWindow().close())
          .catch(() => {
            setStatus("验证结果回传失败，请关闭窗口后重试");
          });
      });
      captcha.onError(() => {
        setStatus("人机验证失败，请关闭窗口后重试");
      });
      captcha.onClose(() => {
        void getCurrentWindow().close();
      });
    },
  );
}

void main();
