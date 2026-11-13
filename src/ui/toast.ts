let toastEl: HTMLDivElement | null = null;
let hideTimer: number | null = null;

export interface ToastOptions {
  duration?: number;
  error?: boolean;
}

export function showToast(message: string, options: ToastOptions = {}): void {
  if (!toastEl) {
    toastEl = document.createElement("div");
    toastEl.className = "toast";
    toastEl.setAttribute("role", "status");
    toastEl.setAttribute("aria-live", "polite");
    document.body.append(toastEl);
  }
  toastEl.textContent = message;
  toastEl.classList.toggle("is-error", Boolean(options.error));
  toastEl.classList.toggle("is-long", message.length > 26);
  toastEl.classList.add("is-visible");

  if (hideTimer !== null) window.clearTimeout(hideTimer);
  hideTimer = window.setTimeout(
    () => {
      toastEl?.classList.remove("is-visible");
    },
    options.duration ?? 2200,
  );
}
