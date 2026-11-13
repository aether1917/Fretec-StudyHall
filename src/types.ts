export type Source = "bili" | "netease";

export type Page = "flow" | "settings";

export type OverlayKind = "search" | "lan" | "player" | "clock" | null;

export type Phase = "focus" | "break";

export type TimerStatus = "idle" | "running" | "paused";

export interface TimerSnapshot {
  phase: Phase;
  totalMs: number;
  startedAtMs: number | null;
  pauseOffsetMs: number;
  pausedAtMs: number | null;
  status: TimerStatus;
}

export interface FlowItem {
  id: string;
  source: Source;
  sourceId: string;
  title: string;
  author: string;
  cover: string;
  durationSec: number;
  addedAt: number;
  progress: {
    positionSec: number;
    updatedAt: number;
  };
}

export interface SearchResultItem {
  source: Source;
  sourceId: string;
  title: string;
  author: string;
  cover: string;
  durationSec: number;
}

export interface SearchPage {
  items: SearchResultItem[];
  page: number;
  hasMore: boolean;
  total: number;
}

export interface QualityInfo {
  requested: number;
  actual: number;
  fallback: boolean;
}

export interface MediaPayload {
  itemId: string;
  kind: "video" | "audio";
  title: string;
  author: string;
  cover: string;
  durationSec: number;
  videoUrl: string | null;
  audioUrl: string | null;
  singleUrl: string | null;
  positionSec: number;
  quality: QualityInfo;
  proxyBase: string;
  notice: string | null;
}

export interface QrLogin {
  key: string;
  url: string;
  image: string;
}

export type QrState =
  | "waiting"
  | "scanned"
  | "expired"
  | "success"
  | "error";

export interface QrPoll {
  state: QrState;
  message: string;
  profile: AccountProfile | null;
}

export interface AccountProfile {
  name: string;
  avatar: string;
}

export interface AccountStatus {
  loggedIn: boolean;
  name: string;
  avatar: string;
}

export interface AuthStatus {
  bili: AccountStatus;
  netease: AccountStatus;
}

export interface CaptchaConfig {
  type: string;
  gt: string;
  challenge: string;
  success: number;
}

export interface SmsTicket {
  captchaKey: string;
  message: string;
}

export interface LanSelf {
  id: string;
  name: string;
}

export interface LanPeer {
  id: string;
  name: string;
  online: boolean;
}

export interface LanStatus {
  available: boolean;
  searching: boolean;
  peers: LanPeer[];
  self: LanSelf;
  message: string;
  canConnect: boolean;
  error: string | null;
}

export interface MiniState {
  itemId: string;
  title: string;
  cover: string;
  playing: boolean;
  positionSec: number;
  durationSec: number;
}

export interface LoginBanner {
  source: Source;
  account: string;
}
