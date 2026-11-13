import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AccountProfile,
  AuthStatus,
  CaptchaConfig,
  FlowItem,
  LanStatus,
  MediaPayload,
  QrLogin,
  QrPoll,
  SearchPage,
  SearchResultItem,
  SmsTicket,
  Source,
  TimerSnapshot,
} from "./types";

export class ApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ApiError";
  }
}

export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    let message = "操作失败，请重试";
    if (typeof error === "string" && error.trim()) message = error;
    else if (error instanceof Error && error.message) message = error.message;
    throw new ApiError(message);
  }
}

export function onEvent<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

/* ---------- 学习流 ---------- */
export const flowList = () => call<FlowItem[]>("flow_list");
export const flowAdd = (item: SearchResultItem) =>
  call<FlowItem>("flow_add", { item });
export const flowRemove = (id: string) => call<void>("flow_remove", { id });
export const flowReorder = (ids: string[]) =>
  call<FlowItem[]>("flow_reorder", { ids });
export const flowProgressSave = (
  id: string,
  positionSec: number,
  durationSec: number,
) =>
  call<void>("flow_progress_save", {
    id,
    positionSec,
    durationSec,
  });

/* ---------- 媒体 ---------- */
export const mediaResolve = (itemId: string, quality: number) =>
  call<MediaPayload>("media_resolve", { itemId, quality });
export const mediaClose = () => call<void>("media_close");

/* ---------- 搜索 ---------- */
export const searchBili = (keyword: string, page: number) =>
  call<SearchPage>("search_bili", { keyword, page });
export const searchNetease = (keyword: string, page: number) =>
  call<SearchPage>("search_netease", { keyword, page });

/* ---------- 账号 ---------- */
export const authStatus = () => call<AuthStatus>("auth_status");
export const authLogout = (source: Source) =>
  call<AuthStatus>("auth_logout", { source });

export const biliQrGenerate = () => call<QrLogin>("bili_qr_generate");
export const biliQrPoll = (key: string) =>
  call<QrPoll>("bili_qr_poll", { key });
export const neteaseQrGenerate = () => call<QrLogin>("netease_qr_generate");
export const neteaseQrPoll = (key: string) =>
  call<QrPoll>("netease_qr_poll", { key });

export const biliCaptchaConfig = () =>
  call<CaptchaConfig>("bili_captcha_config");
export const biliCaptchaResult = (validate: string, seccode: string) =>
  call<void>("bili_captcha_result", { validate, seccode });

export const biliLoginPassword = (
  username: string,
  password: string,
  captchaToken: string | null,
) =>
  call<AccountProfile>("bili_login_password", {
    username,
    password,
    captchaToken,
  });

export const biliSmsSend = (
  tel: string,
  cid: string,
  challenge: string,
  validate: string,
  seccode: string,
) =>
  call<SmsTicket>("bili_sms_send", {
    tel,
    cid,
    challenge,
    validate,
    seccode,
  });
export const biliSmsLogin = (
  tel: string,
  cid: string,
  code: string,
  captchaKey: string,
) =>
  call<AccountProfile>("bili_sms_login", {
    tel,
    cid,
    code,
    captchaKey,
  });

export const neteaseSmsSend = (phone: string) =>
  call<void>("netease_sms_send", { phone });
export const neteaseSmsLogin = (phone: string, code: string) =>
  call<AccountProfile>("netease_sms_login", { phone, code });
export const neteaseLoginPassword = (phone: string, password: string) =>
  call<AccountProfile>("netease_login_password", { phone, password });

/* ---------- 设备名 ---------- */
export const deviceGetName = () => call<string>("device_get_name");
export const deviceSetName = (name: string) =>
  call<string>("device_set_name", { name });

/* ---------- 设置、更新与原生能力 ---------- */
export interface AppSettings {
  theme: string;
  closeBehavior: string;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  hasUpdate: boolean;
  url: string;
  notes: string;
}

export const settingsGet = () => call<AppSettings>("settings_get");
export const settingsSet = (payload: {
  theme?: string;
  closeBehavior?: string;
}) => call<AppSettings>("settings_set", payload);
export const appVersion = () => call<string>("app_version");
export const checkUpdate = () => call<UpdateInfo>("check_update");
export const nativeNotificationStatus = () =>
  call<boolean>("native_notification_status");
export const nativeNotificationRequest = () =>
  call<void>("native_notification_request");

/* ---------- 局域网 ---------- */
export const lanStatus = () => call<LanStatus>("lan_status");
export const lanStart = () => call<LanStatus>("lan_start");
export const lanStop = () => call<void>("lan_stop");
export const lanConnect = (peerId: string) =>
  call<void>("lan_connect", { peerId });
export const lanDisconnect = (peerId: string) =>
  call<void>("lan_disconnect", { peerId });

/* ---------- 计时 ---------- */
export const timerGet = () => call<TimerSnapshot>("timer_get");
export const timerStart = () => call<TimerSnapshot>("timer_start");
export const timerPause = () => call<TimerSnapshot>("timer_pause");
export const timerResume = () => call<TimerSnapshot>("timer_resume");
export const timerStop = () => call<TimerSnapshot>("timer_stop");
