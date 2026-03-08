let pendingNavigationController = null;
let pendingRefreshController = null;

const STORAGE_KEYS = {
  theme: "ghprs.theme",
  filters: "ghprs.prFilters.v3",
};

const FILTER_FIELDS = ["status", "title", "author", "sort", "order"];

const BATCH_NAV_COOLDOWN_MS = 450;
let lastNavigationAt = 0;
let timeAgoIntervalStarted = false;

function guardedNavigate(url) {
  const now = Date.now();
  if (now - lastNavigationAt < BATCH_NAV_COOLDOWN_MS) {
    return;
  }
  lastNavigationAt = now;

  if (typeof window.__ghprsNavigate === "function") {
    window.__ghprsNavigate(url);
    return;
  }

  window.location.assign(url);
}

function isPrimaryNavigation(event) {
  return (
    event.button === 0 &&
    !event.defaultPrevented &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

function canInterceptAnchor(anchor) {
  if (anchor.hasAttribute("download")) {
    return false;
  }

  const target = anchor.getAttribute("target");
  if (target && target !== "_self") {
    return false;
  }

  const rel = (anchor.getAttribute("rel") || "").toLowerCase();
  if (rel.includes("external")) {
    return false;
  }

  const href = anchor.getAttribute("href");
  if (!href || href.startsWith("#")) {
    return false;
  }

  const url = new URL(anchor.href, window.location.href);
  if (url.origin !== window.location.origin) {
    return false;
  }

  if (
    url.pathname === window.location.pathname &&
    url.search === window.location.search &&
    url.hash
  ) {
    return false;
  }

  return true;
}

function serializeGetForm(form, submitter) {
  let formData;
  try {
    formData = submitter ? new FormData(form, submitter) : new FormData(form);
  } catch {
    formData = new FormData(form);
  }

  const params = new URLSearchParams();
  for (const [name, value] of formData.entries()) {
    if (typeof value !== "string") {
      continue;
    }
    if (value.trim() === "") {
      continue;
    }
    params.append(name, value);
  }

  return params;
}

function parseNavigationDocument(html) {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const main = doc.querySelector("main.page");
  if (!(main instanceof HTMLElement)) {
    return null;
  }

  return {
    main,
    title: doc.title,
  };
}

function currentMainPage() {
  const main = document.querySelector("main.page");
  return main instanceof HTMLElement ? main : null;
}

function cancelPendingRefresh() {
  if (pendingRefreshController) {
    pendingRefreshController.abort();
    pendingRefreshController = null;
  }
}

function clearRefreshStatus(main) {
  const status = main.querySelector("[data-page-refresh-status]");
  if (status instanceof HTMLElement) {
    status.replaceChildren();
  }
}

function setRefreshStatus(main, message) {
  const status = main.querySelector("[data-page-refresh-status]");
  if (!(status instanceof HTMLElement)) {
    return;
  }

  const alert = document.createElement("div");
  alert.className = "alert alert--error";
  alert.setAttribute("role", "status");
  alert.textContent = message;
  status.replaceChildren(alert);
}

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

function updateThemeToggleLabel(button) {
  const current = normalizeTheme(document.documentElement.dataset.theme || preferredTheme());
  const isDark = current === "dark";
  button.innerHTML = isDark
    ? '<img src="/assets/icons/sun.svg" alt="" aria-hidden="true" /><span class="sr-only">Light mode</span>'
    : '<img src="/assets/icons/moon.svg" alt="" aria-hidden="true" /><span class="sr-only">Dark mode</span>';
  button.setAttribute("aria-label", isDark ? "Switch to light mode" : "Switch to dark mode");
}

function applyTheme(theme) {
  const normalized = normalizeTheme(theme);
  document.documentElement.dataset.theme = normalized;
  try {
    window.localStorage.setItem(STORAGE_KEYS.theme, normalized);
  } catch {
    // ignore
  }

  const button = document.getElementById("theme-toggle");
  if (button instanceof HTMLButtonElement) {
    updateThemeToggleLabel(button);
  }
}

function initializeThemeToggle() {
  const button = document.getElementById("theme-toggle");
  if (!(button instanceof HTMLButtonElement) || button.dataset.bound === "true") {
    return;
  }

  button.dataset.bound = "true";
  updateThemeToggleLabel(button);
  button.addEventListener("click", () => {
    const current = normalizeTheme(document.documentElement.dataset.theme || preferredTheme());
    applyTheme(current === "dark" ? "light" : "dark");
  });
}

function formatRelativeTime(date) {
  const diffMs = date.getTime() - Date.now();
  const absSeconds = Math.abs(Math.round(diffMs / 1000));
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

  if (absSeconds < 60) {
    return formatter.format(Math.round(diffMs / 1000), "second");
  }

  const absMinutes = Math.abs(Math.round(diffMs / 60000));
  if (absMinutes < 60) {
    return formatter.format(Math.round(diffMs / 60000), "minute");
  }

  const absHours = Math.abs(Math.round(diffMs / 3600000));
  if (absHours < 24) {
    return formatter.format(Math.round(diffMs / 3600000), "hour");
  }

  const absDays = Math.abs(Math.round(diffMs / 86400000));
  if (absDays < 30) {
    return formatter.format(Math.round(diffMs / 86400000), "day");
  }

  const absMonths = Math.abs(Math.round(diffMs / 2629800000));
  if (absMonths < 12) {
    return formatter.format(Math.round(diffMs / 2629800000), "month");
  }

  return formatter.format(Math.round(diffMs / 31557600000), "year");
}

function renderTimeAgoElements(root = document) {
  root.querySelectorAll("time[data-time-ago]").forEach((element) => {
    if (!(element instanceof HTMLTimeElement)) {
      return;
    }

    const raw = element.getAttribute("datetime");
    if (!raw) {
      return;
    }

    const date = new Date(raw);
    if (Number.isNaN(date.getTime())) {
      return;
    }

    element.textContent = formatRelativeTime(date);
    element.title = date.toLocaleString();
  });
}

function initializeTimeAgo() {
  renderTimeAgoElements();

  if (timeAgoIntervalStarted) {
    return;
  }

  timeAgoIntervalStarted = true;
  window.setInterval(() => {
    renderTimeAgoElements();
  }, 60000);
}

function getPrFilterForm() {
  const form = document.querySelector("[data-pr-filter-form]");
  return form instanceof HTMLFormElement ? form : null;
}

function hasQueryState() {
  const params = new URLSearchParams(window.location.search);
  if (params.getAll("repo").length > 0) {
    return true;
  }

  return FILTER_FIELDS.some((name) => {
    const value = params.get(name);
    return value !== null && value !== "";
  });
}

function readCurrentFilterState(form) {
  const state = {};
  for (const field of FILTER_FIELDS) {
    const input = form.elements.namedItem(field);
    if (!(input instanceof HTMLInputElement || input instanceof HTMLSelectElement)) {
      continue;
    }
    const value = input.value.trim();
    if (value !== "") {
      state[field] = value;
    }
  }

  const repoSelect = form.querySelector("[data-pr-filter-repos]");
  if (repoSelect instanceof HTMLSelectElement) {
    const repos = Array.from(repoSelect.selectedOptions)
      .map((option) => option.value)
      .filter(Boolean);
    if (repos.length > 0) {
      state.repo = repos;
    }
  }

  return state;
}

function readSavedFilters() {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEYS.filters);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function writeSavedFilters(state) {
  try {
    window.localStorage.setItem(STORAGE_KEYS.filters, JSON.stringify(state));
  } catch {
    // ignore
  }
}

function setFormValues(form, state) {
  for (const field of FILTER_FIELDS) {
    if (!(field in state)) {
      continue;
    }
    const input = form.elements.namedItem(field);
    if (input instanceof HTMLInputElement || input instanceof HTMLSelectElement) {
      input.value = String(state[field]);
    }
  }

  const repoSelect = form.querySelector("[data-pr-filter-repos]");
  if (repoSelect instanceof HTMLSelectElement && Array.isArray(state.repo)) {
    const selected = new Set(state.repo.filter((value) => typeof value === "string"));
    Array.from(repoSelect.options).forEach((option) => {
      option.selected = selected.has(option.value);
    });
  }
}

function queryStringFromState(state) {
  const params = new URLSearchParams();
  for (const field of FILTER_FIELDS) {
    const value = state[field];
    if (typeof value === "string" && value !== "") {
      params.set(field, value);
    }
  }

  if (Array.isArray(state.repo)) {
    state.repo.forEach((repo) => {
      if (typeof repo === "string" && repo.trim() !== "") {
        params.append("repo", repo);
      }
    });
  }

  const query = params.toString();
  return query ? `?${query}` : "";
}

function initializePrFilters() {
  const form = getPrFilterForm();
  if (!(form instanceof HTMLFormElement) || form.dataset.bound === "true") {
    return;
  }

  form.dataset.bound = "true";

  if (hasQueryState()) {
    writeSavedFilters(readCurrentFilterState(form));
  } else {
    const saved = readSavedFilters();
    if (saved) {
      setFormValues(form, saved);
      const query = queryStringFromState(saved);
      const target = `/prs${query}`;
      if (target !== `${window.location.pathname}${window.location.search}`) {
        guardedNavigate(target);
      }
    }
  }

  form.addEventListener("submit", () => {
    writeSavedFilters(readCurrentFilterState(form));
  });

  const resetControl = document.getElementById("pr-filter-reset");
  if (resetControl instanceof HTMLElement) {
    resetControl.addEventListener("click", (event) => {
      event.preventDefault();
      try {
        window.localStorage.removeItem(STORAGE_KEYS.filters);
      } catch {
        // ignore
      }
      guardedNavigate("/prs");
    });
  }
}

function initializeAutoSubmitControls() {
  document.querySelectorAll("select[data-auto-submit='true']").forEach((control) => {
    if (!(control instanceof HTMLSelectElement) || control.dataset.bound === "true") {
      return;
    }

    control.dataset.bound = "true";
    control.addEventListener("change", () => {
      const form = control.closest("form");
      if (form instanceof HTMLFormElement) {
        form.requestSubmit();
      }
    });
  });
}

function initializeDiffTreeButtons() {
  const tree = document.getElementById("diff-file-tree");
  if (!(tree instanceof HTMLElement) || tree.dataset.bound === "true") {
    return;
  }

  tree.dataset.bound = "true";
  tree.querySelectorAll(".file-leaf-link").forEach((button) => {
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    button.addEventListener("click", () => {
      const targetId = button.getAttribute("data-target");
      if (!targetId) {
        return;
      }
      const section = document.getElementById(targetId);
      if (section) {
        section.scrollIntoView({ behavior: "auto", block: "start" });
      }
    });
  });
}

function initializePageUi() {
  initializeThemeToggle();
  initializeTimeAgo();
  initializePrFilters();
  initializeAutoSubmitControls();
  initializeDiffTreeButtons();
}

function applyNavigationDocument(parsed, url, options) {
  const currentMain = currentMainPage();
  if (!(currentMain instanceof HTMLElement)) {
    return false;
  }

  currentMain.replaceWith(parsed.main);
  if (parsed.title && parsed.title.trim() !== "") {
    document.title = parsed.title;
  }

  if (options.replaceHistory) {
    window.history.replaceState({}, "", url);
  } else {
    window.history.pushState({}, "", url);
  }

  if (!options.preserveScroll) {
    if (url.hash) {
      const id = decodeURIComponent(url.hash.slice(1));
      const target = document.getElementById(id);
      if (target) {
        target.scrollIntoView({ behavior: "auto", block: "start" });
        return true;
      }
    }
    window.scrollTo({ top: 0, left: 0, behavior: "auto" });
  }

  initializePageRefresh();
  initializePageUi();

  return true;
}

function setNavigationPending(isPending) {
  document.documentElement.classList.toggle("is-nav-pending", isPending);
}

async function refreshPageData(main = currentMainPage()) {
  cancelPendingRefresh();

  if (!(main instanceof HTMLElement)) {
    return;
  }

  clearRefreshStatus(main);
  if (main.dataset.needsRefresh !== "true") {
    return;
  }

  const refreshPath = main.dataset.refreshPath || window.location.href;
  const refreshUrl = new URL(refreshPath, window.location.href);
  refreshUrl.searchParams.set("nocache", "1");

  const controller = new AbortController();
  pendingRefreshController = controller;

  try {
    const response = await fetch(refreshUrl.toString(), {
      method: "GET",
      credentials: "same-origin",
      headers: {
        "X-Requested-With": "gh-prs-refresh",
      },
      signal: controller.signal,
    });

    const contentType = (response.headers.get("content-type") || "").toLowerCase();
    if (!response.ok) {
      throw new Error(`Refresh failed (${response.status})`);
    }
    if (!contentType.includes("text/html")) {
      throw new Error("Refresh returned an unexpected response.");
    }

    const html = await response.text();
    const parsed = parseNavigationDocument(html);
    if (!parsed) {
      throw new Error("Refresh returned an invalid page.");
    }

    const currentMain = currentMainPage();
    if (!(currentMain instanceof HTMLElement) || currentMain !== main) {
      return;
    }
    if (!window.Idiomorph || typeof window.Idiomorph.morph !== "function") {
      throw new Error("Refresh support is unavailable.");
    }

    window.Idiomorph.morph(currentMain, parsed.main, { morphStyle: "outerHTML" });
    if (parsed.title && parsed.title.trim() !== "") {
      document.title = parsed.title;
    }

    const nextMain = currentMainPage();
    if (nextMain) {
      clearRefreshStatus(nextMain);
      initializePageUi();
      initializePageRefresh(nextMain);
    }
  } catch (error) {
    if (!controller.signal.aborted) {
      const activeMain = currentMainPage();
      if (activeMain) {
        setRefreshStatus(
          activeMain,
          error instanceof Error ? error.message : "Unable to refresh page data.",
        );
      }
    }
  } finally {
    if (pendingRefreshController === controller) {
      pendingRefreshController = null;
    }
  }
}

function initializePageRefresh(main = currentMainPage()) {
  if (!(main instanceof HTMLElement)) {
    return;
  }

  queueMicrotask(() => {
    if (currentMainPage() === main) {
      void refreshPageData(main);
    }
  });
}

async function navigate(urlLike, options = {}) {
  const url = new URL(urlLike, window.location.href);
  const replaceHistory = options.replaceHistory === true;
  const preserveScroll = options.preserveScroll === true;

  cancelPendingRefresh();

  if (pendingNavigationController) {
    pendingNavigationController.abort();
  }

  const controller = new AbortController();
  pendingNavigationController = controller;
  setNavigationPending(true);

  try {
    const response = await fetch(url.toString(), {
      method: "GET",
      credentials: "same-origin",
      headers: {
        "X-Requested-With": "gh-prs-nav",
      },
      signal: controller.signal,
    });

    const contentType = (response.headers.get("content-type") || "").toLowerCase();
    if (!response.ok || !contentType.includes("text/html")) {
      window.location.assign(url.toString());
      return;
    }

    const html = await response.text();
    const parsed = parseNavigationDocument(html);
    if (!parsed) {
      window.location.assign(url.toString());
      return;
    }

    const finalUrl = new URL(response.url || url.toString(), window.location.href);
    if (!applyNavigationDocument(parsed, finalUrl, { replaceHistory, preserveScroll })) {
      window.location.assign(url.toString());
    }
  } catch (error) {
    if (!controller.signal.aborted) {
      window.location.assign(url.toString());
    }
  } finally {
    if (pendingNavigationController === controller) {
      pendingNavigationController = null;
    }
    setNavigationPending(false);
  }
}

window.__ghprsNavigate = (url, options = {}) => {
  void navigate(url, options);
};

document.addEventListener("click", (event) => {
  if (!isPrimaryNavigation(event)) {
    return;
  }

  const anchor = event.target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)) {
    return;
  }

  if (!canInterceptAnchor(anchor)) {
    return;
  }

  event.preventDefault();
  void navigate(anchor.href);
});

document.addEventListener("submit", (event) => {
  const form = event.target;
  if (!(form instanceof HTMLFormElement)) {
    return;
  }

  const method = (form.getAttribute("method") || "get").toUpperCase();
  if (method !== "GET") {
    return;
  }

  const actionUrl = new URL(form.getAttribute("action") || window.location.href, window.location.href);
  if (actionUrl.origin !== window.location.origin) {
    return;
  }

  event.preventDefault();

  const submitter = event.submitter instanceof HTMLElement ? event.submitter : undefined;
  actionUrl.search = serializeGetForm(form, submitter).toString();
  actionUrl.hash = "";
  void navigate(actionUrl.toString());
});

window.addEventListener("popstate", () => {
  void navigate(window.location.href, {
    force: true,
    replaceHistory: true,
    preserveScroll: true,
  });
});

initializePageUi();
initializePageRefresh();
