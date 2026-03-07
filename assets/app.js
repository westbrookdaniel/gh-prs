let pendingNavigationController = null;
let pendingRefreshController = null;

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

  const alert = document.createElement("cp-alert");
  alert.setAttribute("tone", "error");
  alert.textContent = message;
  status.replaceChildren(alert);
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

void import("/assets/components/index.js");

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

document.addEventListener(
  "error",
  (event) => {
    const target = event.target;
    if (!(target instanceof HTMLImageElement)) {
      return;
    }
    if (!target.closest(".pr-author-avatar")) {
      return;
    }
    const avatar = target.closest(".pr-author-avatar");
    if (avatar) {
      avatar.classList.add("is-fallback");
    }
  },
  true,
);

initializePageRefresh();
