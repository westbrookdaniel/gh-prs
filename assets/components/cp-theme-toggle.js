import { STORAGE_KEYS } from "./runtime.js";

function normalizeTheme(value) {
  return value === "dark" ? "dark" : "light";
}

function preferredTheme() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEYS.theme);
    if (saved === "light" || saved === "dark") {
      return saved;
    }
  } catch {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(theme) {
  const normalized = normalizeTheme(theme);
  document.documentElement.dataset.theme = normalized;
  try {
    window.localStorage.setItem(STORAGE_KEYS.theme, normalized);
  } catch {
    // ignore
  }
}

applyTheme(preferredTheme());

class CpThemeToggle extends HTMLElement {
  connectedCallback() {
    this.innerHTML = "";

    const button = document.createElement("button");
    button.className = "cp-theme-toggle";
    button.type = "button";
    button.addEventListener("click", () => {
      const current = normalizeTheme(document.documentElement.dataset.theme || "light");
      applyTheme(current === "dark" ? "light" : "dark");
      this.renderLabel(button);
    });

    this.appendChild(button);
    this.renderLabel(button);
  }

  renderLabel(button) {
    const current = normalizeTheme(document.documentElement.dataset.theme || "light");
    const isDark = current === "dark";
    button.innerHTML = isDark
      ? '<img src="/assets/icons/moon.svg" alt="" aria-hidden="true" /><span class="sr-only">Dark mode</span>'
      : '<img src="/assets/icons/sun.svg" alt="" aria-hidden="true" /><span class="sr-only">Light mode</span>';
    button.setAttribute("aria-label", isDark ? "Dark mode" : "Light mode");
  }
}

if (!customElements.get("cp-theme-toggle")) {
  customElements.define("cp-theme-toggle", CpThemeToggle);
}
