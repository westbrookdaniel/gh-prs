const STORAGE_KEYS = {
  theme: "ghprs.theme",
  filters: "ghprs.prFilters.v3",
};

const FILTER_FIELDS = ["status", "title", "author", "sort", "order"];

const BATCH_NAV_COOLDOWN_MS = 450;
let lastNavigationAt = 0;

function guardedNavigate(url) {
  const now = Date.now();
  if (now - lastNavigationAt < BATCH_NAV_COOLDOWN_MS) {
    return;
  }
  lastNavigationAt = now;
  window.location.assign(url);
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
    const isDark = current === "dark";
    button.innerHTML = isDark
      ? '<img src="/assets/icons/moon.svg" alt="" aria-hidden="true" /><span class="sr-only">Dark mode</span>'
      : '<img src="/assets/icons/sun.svg" alt="" aria-hidden="true" /><span class="sr-only">Light mode</span>';
    button.setAttribute("aria-label", isDark ? "Dark mode" : "Light mode");
  }
}

class ComboboxComp extends HTMLElement {
  connectedCallback() {
    this.inputName = this.getAttribute("input-name") || "repo";
    this.sourceSelector = this.getAttribute("source-selector") || "";
    this.placeholder = this.getAttribute("placeholder") || "Search repos...";
    this.emptyLabel = this.getAttribute("empty-label") || "All options";
    this.noResultsLabel = this.getAttribute("no-results-label") || "No matches";
    this.single = this.classList.contains("single") || this.getAttribute("single") === "true";
    this.autoSubmit = this.getAttribute("auto-submit") === "true";
    this.formId = this.getAttribute("form-id") || "";
    this.options = [];
    this.selected = new Set();
    this.activeIndex = -1;

    this.render();
    this.initializeOptions();
  }

  initializeOptions() {
    if (this.loadOptionsFromSibling()) {
      return;
    }

    let attempts = 0;
    const retry = () => {
      if (this.loadOptionsFromSibling()) {
        return;
      }
      attempts += 1;
      if (attempts < 12) {
        window.requestAnimationFrame(retry);
      }
    };

    window.requestAnimationFrame(retry);
  }

  render() {
    this.classList.add("repo-multiselect", "combobox-comp");
    this.innerHTML = `
      <button type="button" class="repo-popover-trigger" aria-expanded="false">
        <span class="repo-popover-label" data-trigger-label>${this.emptyLabel}</span>
        <span aria-hidden="true">▾</span>
      </button>
      <div class="repo-popover" hidden>
        <div class="repo-combobox-shell">
          <div class="repo-selected" data-selected></div>
          <input type="text" class="repo-combobox-input" placeholder="${this.placeholder}" aria-label="Search repositories" />
        </div>
        <div class="repo-combobox-list" hidden></div>
      </div>
    `;

    this.trigger = this.querySelector(".repo-popover-trigger");
    this.triggerLabel = this.querySelector("[data-trigger-label]");
    this.popoverEl = this.querySelector(".repo-popover");
    this.input = this.querySelector(".repo-combobox-input");
    this.list = this.querySelector(".repo-combobox-list");
    this.selectedContainer = this.querySelector("[data-selected]");

    this.sourceSelect = null;

    this.trigger.addEventListener("click", () => {
      this.togglePopover();
    });

    this.input.addEventListener("focus", () => {
      this.openPopover();
      this.renderList();
    });
    this.input.addEventListener("input", () => this.renderList());
    this.input.addEventListener("keydown", (event) => this.onKeyDown(event));

    document.addEventListener("click", (event) => {
      if (!this.contains(event.target)) {
        this.closePopover();
      }
    });
  }

  openPopover() {
    this.popoverEl.hidden = false;
    this.trigger.setAttribute("aria-expanded", "true");
  }

  closePopover() {
    this.popoverEl.hidden = true;
    this.list.hidden = true;
    this.trigger.setAttribute("aria-expanded", "false");
  }

  togglePopover() {
    if (this.popoverEl.hidden) {
      this.openPopover();
      this.renderList();
      this.input.focus();
    } else {
      this.closePopover();
    }
  }

  loadOptionsFromSibling() {
    if (this.sourceSelector) {
      this.sourceSelect = document.querySelector(this.sourceSelector);
    }

    if (!(this.sourceSelect instanceof HTMLSelectElement)) {
      const root = this.closest("form") || this.parentElement;
      const fallback = root?.querySelector(".repo-option-source");
      if (fallback instanceof HTMLSelectElement) {
        this.sourceSelect = fallback;
      }
    }

    if (!(this.sourceSelect instanceof HTMLSelectElement)) {
      return false;
    }

    const optionEls = this.sourceSelect.querySelectorAll("option");
    this.options = Array.from(optionEls)
      .map((option) => option.value)
      .filter(Boolean);
    this.selected = new Set(
      Array.from(optionEls)
        .filter((option) => option.hasAttribute("selected"))
        .map((option) => option.value),
    );

    this.renderSelected();
    this.syncSourceSelect();
    this.renderList();
    this.closePopover();
    return true;
  }

  filteredOptions() {
    const search = this.input.value.trim().toLowerCase();
    if (!search) {
      return this.options;
    }
    return this.options.filter((value) => value.toLowerCase().includes(search));
  }

  renderList() {
    this.openPopover();
    const filtered = this.filteredOptions();
    this.list.innerHTML = "";
    this.activeIndex = filtered.length === 0 ? -1 : 0;

    if (filtered.length === 0) {
      this.list.hidden = false;
      const empty = document.createElement("div");
      empty.className = "repo-option-empty";
      empty.textContent = this.noResultsLabel;
      this.list.appendChild(empty);
      return;
    }

    filtered.forEach((repo, index) => {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "repo-option";
      row.dataset.repo = repo;
      row.dataset.index = String(index);
      if (index === this.activeIndex) {
        row.classList.add("is-active");
      }

      const checkbox = document.createElement("span");
      checkbox.className = "repo-option-check";
      checkbox.textContent = this.selected.has(repo) ? "✓" : "";
      row.appendChild(checkbox);

      const label = document.createElement("span");
      label.textContent = repo;
      row.appendChild(label);

      row.addEventListener("click", () => {
        this.toggleRepo(repo);
      });

      this.list.appendChild(row);
    });

    this.list.hidden = false;
  }

  onKeyDown(event) {
    const options = Array.from(this.list.querySelectorAll(".repo-option"));
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (options.length === 0) {
        return;
      }
      event.preventDefault();
      const delta = event.key === "ArrowDown" ? 1 : -1;
      this.activeIndex = Math.max(
        0,
        Math.min(options.length - 1, this.activeIndex + delta),
      );
      options.forEach((option, index) =>
        option.classList.toggle("is-active", index === this.activeIndex),
      );
      return;
    }

    if (event.key === "Enter") {
      if (options.length === 0 || this.activeIndex < 0) {
        return;
      }
      event.preventDefault();
      const option = options[this.activeIndex];
      this.toggleRepo(option.dataset.repo || "");
      return;
    }

    if (event.key === "Escape") {
      this.closePopover();
    }
  }

  toggleRepo(repo) {
    if (!repo) {
      return;
    }

    if (this.single) {
      this.selected = new Set([repo]);
      this.renderSelected();
      this.syncSourceSelect();
      if (this.autoSubmit) {
        this.submitConfiguredForm();
      }
      this.closePopover();
      return;
    }

    if (this.selected.has(repo)) {
      this.selected.delete(repo);
    } else {
      this.selected.add(repo);
    }

    this.renderSelected();
    this.syncSourceSelect();
    this.dispatchEvent(new Event("change", { bubbles: true }));
    this.renderList();
  }

  renderSelected() {
    this.selectedContainer.innerHTML = "";
    if (this.selected.size === 0) {
      const empty = document.createElement("span");
      empty.className = "repo-selected-empty";
      empty.textContent = this.emptyLabel;
      this.selectedContainer.appendChild(empty);
      this.triggerLabel.textContent = this.emptyLabel;
      return;
    }

    this.triggerLabel.textContent =
      this.selected.size === 1
        ? Array.from(this.selected)[0]
        : `${this.selected.size} selected`;

    Array.from(this.selected)
      .sort()
      .forEach((repo) => {
        if (this.single) {
          return;
        }
        const chip = document.createElement("button");
        chip.type = "button";
        chip.className = "repo-chip";
        chip.textContent = repo;
        chip.addEventListener("click", () => {
          this.toggleRepo(repo);
        });
        this.selectedContainer.appendChild(chip);
      });
  }

  syncSourceSelect() {
    if (!(this.sourceSelect instanceof HTMLSelectElement)) {
      return;
    }

    this.sourceSelect.name = this.inputName;
    this.sourceSelect.multiple = !this.single;
    this.sourceSelect.querySelectorAll("option").forEach((option) => {
      option.selected = this.selected.has(option.value);
    });

    if (this.single && this.selected.size === 0) {
      const first = this.sourceSelect.querySelector("option");
      if (first) {
        first.selected = true;
        this.selected = new Set([first.value]);
      }
    }
  }

  getSelectedValues() {
    return Array.from(this.selected);
  }

  setSelectedValues(values) {
    const allowed = new Set(this.options);
    const next = Array.isArray(values)
      ? values.filter((value) => typeof value === "string" && allowed.has(value))
      : [];

    this.selected = this.single ? new Set(next.slice(0, 1)) : new Set(next);
    this.renderSelected();
    this.syncSourceSelect();
    this.renderList();
    this.closePopover();
  }

  submitConfiguredForm() {
    let form = null;
    if (this.formId) {
      form = document.getElementById(this.formId);
    }
    if (!(form instanceof HTMLFormElement)) {
      form = this.closest("form");
    }
    if (form instanceof HTMLFormElement) {
      form.requestSubmit();
    }
  }

}

class PrFilterState extends HTMLElement {
  connectedCallback() {
    this.formId = this.getAttribute("form-id");
    this.resetId = this.getAttribute("reset-id");
    this.comboId = this.getAttribute("combo-id");

    this.form = document.getElementById(this.formId);
    this.resetLink = document.getElementById(this.resetId);
    this.combo = document.getElementById(this.comboId);
    this.statusCombo = document.getElementById("status-combobox");

    if (!(this.form instanceof HTMLFormElement)) {
      return;
    }

    this.hydrateFromStorageIfNeeded();
    this.bindSaveState();
    this.bindReset();
    this.bindTreeButtons();
  }

  hasQueryState() {
    const params = new URLSearchParams(window.location.search);
    if (params.getAll("repo").length > 0) {
      return true;
    }

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

    if (this.combo && typeof this.combo.getSelectedValues === "function") {
      const repos = this.combo.getSelectedValues();
      if (repos.length > 0) {
        state.repo = repos;
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
      // ignore
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

    if (
      Array.isArray(state.repo) &&
      this.combo &&
      typeof this.combo.setSelectedValues === "function"
    ) {
      this.combo.setSelectedValues(state.repo);
    }

    if (state.status && this.statusCombo && typeof this.statusCombo.setSelectedValues === "function") {
      this.statusCombo.setSelectedValues([state.status]);
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

  hydrateFromStorageIfNeeded() {
    if (this.hasQueryState()) {
      const current = this.readCurrentState();
      this.writeSavedFilters(current);
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
      guardedNavigate(target);
    }
  }

  bindSaveState() {
    this.form.addEventListener("submit", () => {
      this.writeSavedFilters(this.readCurrentState());
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
        // ignore
      }
      guardedNavigate("/prs");
    });
  }

  bindTreeButtons() {
    const tree = document.getElementById("diff-file-tree");
    if (!tree) {
      return;
    }

    tree.querySelectorAll(".file-leaf-link").forEach((button) => {
      button.addEventListener("click", () => {
        const targetId = button.getAttribute("data-target");
        if (!targetId) {
          return;
        }
        const section = document.getElementById(targetId);
        if (!section) {
          return;
        }
        section.scrollIntoView({ behavior: "smooth", block: "start" });
      });
    });
  }

}

customElements.define("theme-toggle", ThemeToggle);
customElements.define("combobox-comp", ComboboxComp);
customElements.define("pr-filter-state", PrFilterState);

document.addEventListener("error", (event) => {
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
}, true);
