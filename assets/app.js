const STORAGE_KEYS = {
  theme: "ghprs.theme",
  filters: "ghprs.prFilters.v1",
  recentRepos: "ghprs.recentRepos.v1",
};

const FILTER_FIELDS = ["org", "repo", "status", "title", "author", "sort", "order"];

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
    return;
  }
}

applyTheme(preferredTheme());

class ThemeToggle extends HTMLElement {
  connectedCallback() {
    this.innerHTML = "";

    const button = document.createElement("button");
    button.className = "theme-toggle";
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
    button.textContent = current === "dark" ? "Dark mode" : "Light mode";
    button.setAttribute("aria-label", "Toggle dark and light mode");
  }
}

class PrFilterState extends HTMLElement {
  connectedCallback() {
    this.formId = this.getAttribute("form-id");
    this.resetId = this.getAttribute("reset-id");
    this.chipsId = this.getAttribute("chips-id");
    this.datalistId = this.getAttribute("datalist-id");

    this.form = document.getElementById(this.formId);
    this.resetLink = document.getElementById(this.resetId);
    this.chips = document.getElementById(this.chipsId);
    this.datalist = document.getElementById(this.datalistId);

    if (!(this.form instanceof HTMLFormElement)) {
      return;
    }

    this.hydrateFromStorageIfNeeded();
    this.bindSaveState();
    this.bindReset();
    this.renderRecentRepos();
  }

  hasQueryState() {
    const params = new URLSearchParams(window.location.search);
    return FILTER_FIELDS.some((name) => {
      const value = params.get(name);
      return value !== null && value !== "";
    });
  }

  readCurrentState() {
    const state = {};
    for (const field of FILTER_FIELDS) {
      const input = this.form.elements.namedItem(field);
      if (!(input instanceof HTMLInputElement || input instanceof HTMLSelectElement)) {
        continue;
      }
      const value = input.value.trim();
      if (value !== "") {
        state[field] = value;
      }
    }
    return state;
  }

  readSavedFilters() {
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

  writeSavedFilters(state) {
    try {
      window.localStorage.setItem(STORAGE_KEYS.filters, JSON.stringify(state));
    } catch {
      return;
    }
  }

  readRecentRepos() {
    try {
      const raw = window.localStorage.getItem(STORAGE_KEYS.recentRepos);
      if (!raw) {
        return [];
      }
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        return [];
      }
      return parsed.filter((value) => typeof value === "string" && value.trim() !== "");
    } catch {
      return [];
    }
  }

  writeRecentRepos(repo) {
    const normalized = repo.trim();
    if (!normalized.includes("/")) {
      return;
    }

    const existing = this.readRecentRepos().filter(
      (value) => value.toLowerCase() !== normalized.toLowerCase(),
    );
    existing.unshift(normalized);
    const trimmed = existing.slice(0, 8);

    try {
      window.localStorage.setItem(STORAGE_KEYS.recentRepos, JSON.stringify(trimmed));
    } catch {
      return;
    }
  }

  setFormValues(state) {
    for (const field of FILTER_FIELDS) {
      if (!(field in state)) {
        continue;
      }
      const input = this.form.elements.namedItem(field);
      if (input instanceof HTMLInputElement || input instanceof HTMLSelectElement) {
        input.value = String(state[field]);
      }
    }
  }

  queryStringFromState(state) {
    const params = new URLSearchParams();
    for (const field of FILTER_FIELDS) {
      const value = state[field];
      if (typeof value === "string" && value !== "") {
        params.set(field, value);
      }
    }
    const query = params.toString();
    return query ? `?${query}` : "";
  }

  hydrateFromStorageIfNeeded() {
    if (this.hasQueryState()) {
      const current = this.readCurrentState();
      this.writeSavedFilters(current);
      if (current.repo) {
        this.writeRecentRepos(current.repo);
      }
      return;
    }

    const saved = this.readSavedFilters();
    if (!saved) {
      return;
    }

    this.setFormValues(saved);
    const query = this.queryStringFromState(saved);
    const target = `/prs${query}`;
    if (target !== `${window.location.pathname}${window.location.search}`) {
      window.location.replace(target);
    }
  }

  bindSaveState() {
    this.form.addEventListener("submit", () => {
      const state = this.readCurrentState();
      this.writeSavedFilters(state);
      if (state.repo) {
        this.writeRecentRepos(state.repo);
      }
    });
  }

  bindReset() {
    if (!(this.resetLink instanceof HTMLAnchorElement)) {
      return;
    }

    this.resetLink.addEventListener("click", (event) => {
      event.preventDefault();
      try {
        window.localStorage.removeItem(STORAGE_KEYS.filters);
      } catch {
        // no-op
      }
      window.location.assign("/prs");
    });
  }

  renderRecentRepos() {
    const repos = this.readRecentRepos();

    if (this.datalist instanceof HTMLDataListElement) {
      this.datalist.innerHTML = "";
      for (const repo of repos) {
        const option = document.createElement("option");
        option.value = repo;
        this.datalist.appendChild(option);
      }
    }

    if (!(this.chips instanceof HTMLElement)) {
      return;
    }

    this.chips.innerHTML = "";
    if (repos.length === 0) {
      this.chips.hidden = true;
      return;
    }

    this.chips.hidden = false;
    for (const repo of repos) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "chip";
      button.textContent = repo;
      button.addEventListener("click", () => {
        const input = this.form.elements.namedItem("repo");
        if (input instanceof HTMLInputElement) {
          input.value = repo;
          input.focus();
        }
      });
      this.chips.appendChild(button);
    }
  }
}

customElements.define("theme-toggle", ThemeToggle);
customElements.define("pr-filter-state", PrFilterState);
