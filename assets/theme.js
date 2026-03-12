// @ts-check

/** @typedef {"light" | "dark"} Theme */

const THEME_STORAGE_KEY = "ghprs.theme";

/**
 * @param {string | undefined} value
 * @returns {Theme}
 */
export function normalizeTheme(value) {
  return value === "dark" ? "dark" : "light";
}

/**
 * @returns {Theme}
 */
export function preferredTheme() {
  try {
    const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (saved === "light" || saved === "dark") {
      return saved;
    }
  } catch {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/**
 * @param {HTMLButtonElement} button
 */
function updateThemeToggleLabel(button) {
  const current = normalizeTheme(
    document.documentElement.dataset.theme || preferredTheme(),
  );
  const isDark = current === "dark";

  button.innerHTML = isDark
    ? '<img src="/assets/icons/sun.svg" alt="" aria-hidden="true" /><span class="sr-only">Light mode</span>'
    : '<img src="/assets/icons/moon.svg" alt="" aria-hidden="true" /><span class="sr-only">Dark mode</span>';
  button.setAttribute(
    "aria-label",
    isDark ? "Switch to light mode" : "Switch to dark mode",
  );
}

/**
 * @param {string} theme
 */
export function applyTheme(theme) {
  const normalized = normalizeTheme(theme);
  document.documentElement.dataset.theme = normalized;

  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, normalized);
  } catch {
    // ignore
  }

  const button = document.getElementById("theme-toggle");
  if (button instanceof HTMLButtonElement) {
    updateThemeToggleLabel(button);
  }
}

export function initializeThemeToggle() {
  const button = document.getElementById("theme-toggle");
  if (
    !(button instanceof HTMLButtonElement) ||
    button.dataset.bound === "true"
  ) {
    return;
  }

  button.dataset.bound = "true";
  updateThemeToggleLabel(button);
  button.addEventListener("click", () => {
    const current = normalizeTheme(
      document.documentElement.dataset.theme || preferredTheme(),
    );
    applyTheme(current === "dark" ? "light" : "dark");
  });
}
