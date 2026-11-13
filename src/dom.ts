type AttrValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | EventListener;

export interface ElementProps {
  [key: string]: AttrValue | Record<string, unknown> | undefined;
  class?: string;
  style?: string;
  dataset?: Record<string, string>;
  on?: Record<string, EventListener>;
}

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: ElementProps | null = null,
  ...children: Array<Node | string | number | null | undefined | false>
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (props) {
    for (const [key, value] of Object.entries(props)) {
      if (value === undefined || value === null || value === false) continue;
      if (key === "class") {
        el.className = String(value);
      } else if (key === "style") {
        el.setAttribute("style", String(value));
      } else if (key === "dataset") {
        for (const [dk, dv] of Object.entries(value as Record<string, string>)) {
          el.dataset[dk] = dv;
        }
      } else if (key === "on") {
        for (const [event, handler] of Object.entries(
          value as Record<string, EventListener>,
        )) {
          el.addEventListener(event, handler);
        }
      } else if (key === "html") {
        el.innerHTML = String(value);
      } else {
        el.setAttribute(key, String(value));
      }
    }
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    el.append(
      typeof child === "string" || typeof child === "number"
        ? document.createTextNode(String(child))
        : child,
    );
  }
  return el;
}

export function clear(el: Element): void {
  while (el.firstChild) el.removeChild(el.firstChild);
}

export function qs<T extends Element = HTMLElement>(
  selector: string,
  root: ParentNode = document,
): T {
  const el = root.querySelector<T>(selector);
  if (!el) throw new Error(`missing element: ${selector}`);
  return el;
}

export function mount(container: Element, ...nodes: Node[]): void {
  clear(container);
  container.append(...nodes);
}

export function setClass(el: Element, name: string, active: boolean): void {
  el.classList.toggle(name, active);
}

/** 创建 SVG 元素（用于内联图标） */
export function svgIcon(path: string, viewBox = "0 0 24 24"): SVGSVGElement {
  const wrap = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  wrap.setAttribute("viewBox", viewBox);
  wrap.setAttribute("aria-hidden", "true");
  wrap.setAttribute("focusable", "false");
  wrap.innerHTML = path;
  return wrap;
}
