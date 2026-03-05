const STORAGE_KEYS = {
  theme: "ghprs.theme",
  filters: "ghprs.prFilters.v3",
};

const FILTER_FIELDS = ["status", "title", "author", "sort", "order"];

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

class RepoMultiSelect extends HTMLElement {
  connectedCallback() {
    this.inputName = this.getAttribute("input-name") || "repo";
    this.placeholder = this.getAttribute("placeholder") || "Search repos...";
    this.options = [];
    this.selected = new Set();
    this.activeIndex = -1;

    this.render();
    this.loadOptionsFromSibling();
  }

  render() {
    this.classList.add("repo-multiselect");
    this.innerHTML = `
      <button type="button" class="repo-popover-trigger" aria-expanded="false">
        <span class="repo-popover-label" data-trigger-label>All repos</span>
        <span aria-hidden="true">▾</span>
      </button>
      <div class="repo-popover" hidden>
        <div class="repo-combobox-shell">
          <div class="repo-selected" data-selected></div>
          <input type="text" class="repo-combobox-input" placeholder="${this.placeholder}" aria-label="Search repositories" />
        </div>
        <div class="repo-combobox-list" hidden></div>
      </div>
      <div class="repo-hidden-inputs"></div>
    `;

    this.trigger = this.querySelector(".repo-popover-trigger");
    this.triggerLabel = this.querySelector("[data-trigger-label]");
    this.popover = this.querySelector(".repo-popover");
    this.input = this.querySelector(".repo-combobox-input");
    this.list = this.querySelector(".repo-combobox-list");
    this.selectedContainer = this.querySelector("[data-selected]");
    this.hiddenInputs = this.querySelector(".repo-hidden-inputs");

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
    this.popover.hidden = false;
    this.trigger.setAttribute("aria-expanded", "true");
  }

  closePopover() {
    this.popover.hidden = true;
    this.list.hidden = true;
    this.trigger.setAttribute("aria-expanded", "false");
  }

  togglePopover() {
    if (this.popover.hidden) {
      this.openPopover();
      this.renderList();
      this.input.focus();
    } else {
      this.closePopover();
    }
  }

  loadOptionsFromSibling() {
    const source = this.parentElement?.querySelector(".repo-option-source");
    if (!source) {
      return;
    }

    const optionEls = source.querySelectorAll("option");
    this.options = Array.from(optionEls)
      .map((option) => option.value)
      .filter(Boolean);
    this.selected = new Set(
      Array.from(optionEls)
        .filter((option) => option.hasAttribute("selected"))
        .map((option) => option.value),
    );

    this.renderSelected();
    this.syncHiddenInputs();
    this.renderList();
    this.closePopover();
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
      empty.textContent = "No repositories found";
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

    if (this.selected.has(repo)) {
      this.selected.delete(repo);
    } else {
      this.selected.add(repo);
    }

    this.renderSelected();
    this.syncHiddenInputs();
    this.renderList();
  }

  renderSelected() {
    this.selectedContainer.innerHTML = "";
    if (this.selected.size === 0) {
      const empty = document.createElement("span");
      empty.className = "repo-selected-empty";
      empty.textContent = "All accessible repos";
      this.selectedContainer.appendChild(empty);
      this.triggerLabel.textContent = "All repos";
      return;
    }

    this.triggerLabel.textContent =
      this.selected.size === 1
        ? Array.from(this.selected)[0]
        : `${this.selected.size} repos`;

    Array.from(this.selected)
      .sort()
      .forEach((repo) => {
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

  syncHiddenInputs() {
    this.hiddenInputs.innerHTML = "";
    Array.from(this.selected)
      .sort()
      .forEach((repo) => {
        const input = document.createElement("input");
        input.type = "hidden";
        input.name = this.inputName;
        input.value = repo;
        this.hiddenInputs.appendChild(input);
      });
  }

  getSelectedRepos() {
    return Array.from(this.selected);
  }

  setSelectedRepos(repos) {
    this.selected = new Set(repos);
    this.renderSelected();
    this.syncHiddenInputs();
    this.renderList();
    this.closePopover();
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

    if (this.combo && typeof this.combo.getSelectedRepos === "function") {
      const repos = this.combo.getSelectedRepos();
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
      typeof this.combo.setSelectedRepos === "function"
    ) {
      this.combo.setSelectedRepos(state.repo);
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
      window.location.replace(target);
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
      window.location.assign("/prs");
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
customElements.define("repo-multiselect", RepoMultiSelect);
customElements.define("pr-filter-state", PrFilterState);
