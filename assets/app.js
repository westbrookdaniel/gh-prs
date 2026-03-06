let pendingNavigationController = null;

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

function applyNavigationDocument(parsed, url, options) {
  const currentMain = document.querySelector("main.page");
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

  return true;
}

function setNavigationPending(isPending) {
  document.documentElement.classList.toggle("is-nav-pending", isPending);
}

async function navigate(urlLike, options = {}) {
  const url = new URL(urlLike, window.location.href);
  const replaceHistory = options.replaceHistory === true;
  const preserveScroll = options.preserveScroll === true;

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
