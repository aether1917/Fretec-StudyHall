import type { TimerSnapshot } from "../types";

export function remainingMs(snapshot: TimerSnapshot, now = Date.now()): number {
  if (snapshot.status === "idle" || snapshot.startedAtMs === null) {
    return snapshot.totalMs;
  }
  const anchor = snapshot.pausedAtMs ?? now;
  const elapsed = anchor - snapshot.startedAtMs - snapshot.pauseOffsetMs;
  return Math.max(0, snapshot.totalMs - elapsed);
}

export function formatMMSS(ms: number): string {
  const totalSeconds = Math.max(0, Math.ceil(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function formatFlip(ms: number): { minutes: string; seconds: string } {
  const totalSeconds = Math.max(0, Math.ceil(ms / 1000));
  const minutes = Math.min(99, Math.floor(totalSeconds / 60));
  const seconds = totalSeconds % 60;
  return {
    minutes: String(minutes).padStart(2, "0"),
    seconds: String(seconds).padStart(2, "0"),
  };
}

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "00:00";
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hours > 0) {
    return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(
      2,
      "0",
    )}:${String(secs).padStart(2, "0")}`;
  }
  return `${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

export function formatDurationLabel(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  const total = Math.floor(seconds);
  const minutes = Math.floor(total / 60);
  const secs = total % 60;
  return `${minutes}:${String(secs).padStart(2, "0")}`;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
