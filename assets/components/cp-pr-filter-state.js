import { STORAGE_KEYS, guardedNavigate } from "./runtime.js";

const FILTER_FIELDS = ["status", "title", "author", "sort", "order"];

class CpPrFilterState extends HTMLElement {
  connectedCallback() {
    this.formId = this.getAttribute("form-id");
    this.resetId = this.getAttribute("reset-id");
    this.comboId = this.getAttribute("combo-id");

    this.form = document.getElementById(this.formId);
    this.resetControl = document.getElementById(this.resetId);
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
    if (!(this.resetControl instanceof HTMLElement)) {
      return;
    }

    this.resetControl.addEventListener("click", (event) => {
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
        section.scrollIntoView({ behavior: "auto", block: "start" });
      });
    });
  }
}

if (!customElements.get("cp-pr-filter-state")) {
  customElements.define("cp-pr-filter-state", CpPrFilterState);
}
