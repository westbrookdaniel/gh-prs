// @ts-check

import { addGlobalStatus, removeGlobalStatus } from "./status.js";
import { initializeThemeToggle } from "./theme.js";

/** @typedef {HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement} FormControl */
/** @typedef {{ main: HTMLElement, title: string }} ParsedNavigationDocument */
/** @typedef {{ replaceHistory?: boolean, preserveScroll?: boolean, force?: boolean }} NavigateOptions */
/** @typedef {{ status?: string, title?: string, author?: string, sort?: string, order?: string, repo?: string[] }} FilterState */

/** @type {Window & typeof globalThis & {
 *   __ghprsNavigate?: (url: string | URL, options?: NavigateOptions) => void,
 *   Idiomorph?: { morph: (currentNode: Element, newNode: Element, options?: { morphStyle: string }) => void },
 * }} */
const appWindow = window;

/** @type {AbortController | null} */
let pendingNavigationController = null;
/** @type {AbortController | null} */
let pendingRefreshController = null;

const STORAGE_KEYS = {
  filters: "ghprs.prFilters.v3",
};

const FILTER_FIELDS = ["status", "title", "author", "sort", "order"];

const BATCH_NAV_COOLDOWN_MS = 450;
let lastNavigationAt = 0;
let timeAgoIntervalStarted = false;

/**
 * @param {string} url
 */
function guardedNavigate(url) {
  const now = Date.now();
  if (now - lastNavigationAt < BATCH_NAV_COOLDOWN_MS) {
    return;
  }
  lastNavigationAt = now;

  if (typeof appWindow.__ghprsNavigate === "function") {
    appWindow.__ghprsNavigate(url);
    return;
  }

  window.location.assign(url);
}

/**
 * @param {MouseEvent} event
 */
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

/**
 * @param {HTMLAnchorElement} anchor
 */
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

/**
 * @param {HTMLFormElement} form
 * @param {HTMLElement | undefined} submitter
 * @returns {URLSearchParams}
 */
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

/**
 * @param {string} html
 * @returns {ParsedNavigationDocument | null}
 */
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

/**
 * @returns {HTMLElement | null}
 */
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

/**
 * @param {HTMLElement} main
 */
function clearRefreshStatus(main) {
  const status = main.querySelector("[data-page-refresh-status]");
  if (status instanceof HTMLElement) {
    status.replaceChildren();
  }
}

/**
 * @param {HTMLElement} main
 * @param {string} message
 */
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

/**
 * @param {Date} date
 * @returns {string}
 */
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

/**
 * @param {Document | HTMLElement} [root=document]
 */
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

/**
 * @returns {HTMLFormElement | null}
 */
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

/**
 * @param {HTMLFormElement} form
 * @returns {FilterState}
 */
function readCurrentFilterState(form) {
  /** @type {FilterState} */
  const state = {};
  for (const field of FILTER_FIELDS) {
    const input = form.elements.namedItem(field);
    if (
      !(input instanceof HTMLInputElement || input instanceof HTMLSelectElement)
    ) {
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

/**
 * @returns {FilterState | null}
 */
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
    return /** @type {FilterState} */ (parsed);
  } catch {
    return null;
  }
}

/**
 * @param {FilterState} state
 */
function writeSavedFilters(state) {
  try {
    window.localStorage.setItem(STORAGE_KEYS.filters, JSON.stringify(state));
  } catch {
    // ignore
  }
}

/**
 * @param {HTMLFormElement} form
 * @param {FilterState} state
 */
function setFormValues(form, state) {
  for (const field of FILTER_FIELDS) {
    if (!(field in state)) {
      continue;
    }
    const input = form.elements.namedItem(field);
    if (
      input instanceof HTMLInputElement ||
      input instanceof HTMLSelectElement
    ) {
      input.value = String(state[field]);
    }
  }

  const repoSelect = form.querySelector("[data-pr-filter-repos]");
  if (repoSelect instanceof HTMLSelectElement && Array.isArray(state.repo)) {
    const selected = new Set(
      state.repo.filter((value) => typeof value === "string"),
    );
    Array.from(repoSelect.options).forEach((option) => {
      option.selected = selected.has(option.value);
    });
  }
}

/**
 * @param {FilterState} state
 * @returns {string}
 */
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
  document
    .querySelectorAll("select[data-auto-submit='true']")
    .forEach((control) => {
      if (
        !(control instanceof HTMLSelectElement) ||
        control.dataset.bound === "true"
      ) {
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

/**
 * @param {ParsedNavigationDocument} parsed
 * @param {URL} url
 * @param {NavigateOptions} options
 * @returns {boolean}
 */
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

/**
 * @param {boolean} isPending
 */
function setNavigationPending(isPending) {
  document.documentElement.classList.toggle("is-nav-pending", isPending);
}

/**
 * @param {HTMLElement | null} [main=currentMainPage()]
 */
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
  addGlobalStatus("refresh", "Refreshing");

  try {
    const response = await fetch(refreshUrl.toString(), {
      method: "GET",
      credentials: "same-origin",
      headers: {
        "X-Requested-With": "gh-prs-refresh",
      },
      signal: controller.signal,
    });

    const contentType = (
      response.headers.get("content-type") || ""
    ).toLowerCase();
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
    if (
      !appWindow.Idiomorph ||
      typeof appWindow.Idiomorph.morph !== "function"
    ) {
      throw new Error("Refresh support is unavailable.");
    }

    appWindow.Idiomorph.morph(currentMain, parsed.main, {
      morphStyle: "outerHTML",
    });
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
          error instanceof Error
            ? error.message
            : "Unable to refresh page data.",
        );
      }
    }
  } finally {
    removeGlobalStatus("refresh");
    if (pendingRefreshController === controller) {
      pendingRefreshController = null;
    }
  }
}

/**
 * @param {HTMLElement | null} [main=currentMainPage()]
 */
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

/**
 * @param {HTMLFormElement} form
 */
function nativeSubmitForm(form) {
  HTMLFormElement.prototype.submit.call(form);
}

/**
 * @param {HTMLFormElement} form
 * @param {HTMLElement | undefined} submitter
 * @returns {string}
 */
function formStatusMessage(form, submitter) {
  if (submitter instanceof HTMLElement) {
    const submitterMessage = submitter.getAttribute("data-status-message");
    if (
      typeof submitterMessage === "string" &&
      submitterMessage.trim() !== ""
    ) {
      return submitterMessage.trim();
    }
  }

  const formMessage = form.getAttribute("data-status-message");
  if (typeof formMessage === "string" && formMessage.trim() !== "") {
    return formMessage.trim();
  }

  return "Submitting";
}

/**
 * @param {HTMLFormElement} form
 * @returns {string}
 */
function formStatusKey(form) {
  const action = form.getAttribute("action") || window.location.pathname;
  return `submit:${action}`;
}

/**
 * @param {HTMLFormElement} form
 * @param {HTMLElement | undefined} submitter
 * @returns {() => void}
 */
function beginFormSubmissionState(form, submitter) {
  /** @type {FormControl[]} */
  const controls = [];
  for (const element of Array.from(form.elements)) {
    if (
      element instanceof HTMLButtonElement ||
      element instanceof HTMLInputElement ||
      element instanceof HTMLSelectElement ||
      element instanceof HTMLTextAreaElement
    ) {
      controls.push(element);
    }
  }

  const snapshot = controls.map((control) => ({
    control,
    disabled: control.disabled,
    text: control instanceof HTMLButtonElement ? control.textContent : null,
  }));

  form.dataset.submitting = "true";
  controls.forEach((control) => {
    control.disabled = true;
  });

  if (submitter instanceof HTMLButtonElement) {
    const loadingLabel = submitter.getAttribute("data-loading-label");
    if (typeof loadingLabel === "string" && loadingLabel.trim() !== "") {
      submitter.textContent = loadingLabel.trim();
    }
  }

  return () => {
    delete form.dataset.submitting;
    snapshot.forEach(({ control, disabled, text }) => {
      control.disabled = disabled;
      if (control instanceof HTMLButtonElement && typeof text === "string") {
        control.textContent = text;
      }
    });
  };
}

/**
 * @param {HTMLFormElement} form
 * @param {HTMLElement | undefined} submitter
 */
async function submitPostForm(form, submitter) {
  const actionUrl = new URL(
    form.getAttribute("action") || window.location.href,
    window.location.href,
  );
  if (actionUrl.origin !== window.location.origin) {
    nativeSubmitForm(form);
    return;
  }

  cancelPendingRefresh();

  const statusKey = formStatusKey(form);
  addGlobalStatus(statusKey, formStatusMessage(form, submitter));
  const endSubmissionState = beginFormSubmissionState(form, submitter);

  let formData;
  try {
    formData = submitter ? new FormData(form, submitter) : new FormData(form);
  } catch {
    formData = new FormData(form);
  }

  try {
    const response = await fetch(actionUrl.toString(), {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "X-Requested-With": "gh-prs-submit",
      },
      body: formData,
      redirect: "follow",
    });

    const contentType = (
      response.headers.get("content-type") || ""
    ).toLowerCase();
    if (!response.ok || !contentType.includes("text/html")) {
      window.location.assign(response.url || actionUrl.toString());
      return;
    }

    const html = await response.text();
    const parsed = parseNavigationDocument(html);
    if (!parsed) {
      window.location.assign(response.url || actionUrl.toString());
      return;
    }

    const finalUrl = new URL(
      response.url || actionUrl.toString(),
      window.location.href,
    );
    if (
      !applyNavigationDocument(parsed, finalUrl, {
        replaceHistory: false,
        preserveScroll: false,
      })
    ) {
      window.location.assign(finalUrl.toString());
    }
  } catch {
    nativeSubmitForm(form);
  } finally {
    endSubmissionState();
    removeGlobalStatus(statusKey);
  }
}

/**
 * @param {string | URL} urlLike
 * @param {NavigateOptions} [options={}]
 */
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

    const contentType = (
      response.headers.get("content-type") || ""
    ).toLowerCase();
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

    const finalUrl = new URL(
      response.url || url.toString(),
      window.location.href,
    );
    if (
      !applyNavigationDocument(parsed, finalUrl, {
        replaceHistory,
        preserveScroll,
      })
    ) {
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

appWindow.__ghprsNavigate = (url, options = {}) => {
  void navigate(url, options);
};

/**
 * @param {MouseEvent} event
 */
function handleDocumentClick(event) {
  if (!isPrimaryNavigation(event)) {
    return;
  }

  if (!(event.target instanceof Element)) {
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
}

/**
 * @param {SubmitEvent} event
 */
function handleDocumentSubmit(event) {
  const form = event.target;
  if (!(form instanceof HTMLFormElement)) {
    return;
  }

  const method = (form.getAttribute("method") || "get").toUpperCase();
  if (method === "POST") {
    event.preventDefault();
    const submitter =
      event.submitter instanceof HTMLElement ? event.submitter : undefined;
    void submitPostForm(form, submitter);
    return;
  }

  if (method !== "GET") {
    return;
  }

  const actionUrl = new URL(
    form.getAttribute("action") || window.location.href,
    window.location.href,
  );
  if (actionUrl.origin !== window.location.origin) {
    return;
  }

  event.preventDefault();

  const submitter =
    event.submitter instanceof HTMLElement ? event.submitter : undefined;
  actionUrl.search = serializeGetForm(form, submitter).toString();
  actionUrl.hash = "";
  void navigate(actionUrl.toString());
}

document.addEventListener("click", handleDocumentClick);
document.addEventListener("submit", handleDocumentSubmit);

window.addEventListener("popstate", () => {
  void navigate(window.location.href, {
    force: true,
    replaceHistory: true,
    preserveScroll: true,
  });
});

initializePageUi();
initializePageRefresh();
