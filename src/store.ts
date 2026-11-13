import type {
  AuthStatus,
  FlowItem,
  LanStatus,
  LoginBanner,
  MiniState,
  OverlayKind,
  Page,
  TimerSnapshot,
} from "./types";

export interface AppState {
  page: Page;
  overlay: OverlayKind;
  flow: FlowItem[];
  timer: TimerSnapshot;
  auth: AuthStatus;
  mini: MiniState | null;
  lan: LanStatus | null;
  banner: LoginBanner | null;
  /** 设置页需要高亮的账号卡（来自登录过期提示条） */
  highlight: "bili" | "netease" | null;
}

type Listener = (state: AppState) => void;

export const initialState: AppState = {
  page: "flow",
  overlay: null,
  flow: [],
  timer: {
    phase: "focus",
    totalMs: 25 * 60 * 1000,
    startedAtMs: null,
    pauseOffsetMs: 0,
    pausedAtMs: null,
    status: "idle",
  },
  auth: {
    bili: { loggedIn: false, name: "", avatar: "" },
    netease: { loggedIn: false, name: "", avatar: "" },
  },
  mini: null,
  lan: null,
  banner: null,
  highlight: null,
};

class Store {
  private state: AppState = { ...initialState };
  private listeners = new Set<Listener>();

  get(): AppState {
    return this.state;
  }

  set(patch: Partial<AppState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener(this.state);
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export const store = new Store();
