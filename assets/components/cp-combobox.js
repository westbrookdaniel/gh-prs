class CpCombobox extends HTMLElement {
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
    this.classList.add("cp-combobox", "repo-multiselect");
    this.innerHTML = `
      <button type="button" class="cp-combobox-trigger" aria-expanded="false">
        <span class="cp-combobox-label" data-trigger-label>${this.emptyLabel}</span>
        <span aria-hidden="true">▾</span>
      </button>
      <div class="cp-combobox-popover" hidden>
        <div class="cp-combobox-shell">
          <div class="cp-combobox-selected" data-selected></div>
          <input type="text" class="cp-combobox-input" placeholder="${this.placeholder}" aria-label="Search repositories" />
        </div>
        <div class="cp-combobox-list" hidden></div>
      </div>
    `;

    this.trigger = this.querySelector(".cp-combobox-trigger");
    this.triggerLabel = this.querySelector("[data-trigger-label]");
    this.popoverEl = this.querySelector(".cp-combobox-popover");
    this.input = this.querySelector(".cp-combobox-input");
    this.list = this.querySelector(".cp-combobox-list");
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
      empty.className = "cp-combobox-option-empty";
      empty.textContent = this.noResultsLabel;
      this.list.appendChild(empty);
      return;
    }

    filtered.forEach((repo, index) => {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "cp-combobox-option";
      row.dataset.repo = repo;
      row.dataset.index = String(index);
      if (index === this.activeIndex) {
        row.classList.add("is-active");
      }

      const checkbox = document.createElement("span");
      checkbox.className = "cp-combobox-option-check";
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
    const options = Array.from(this.list.querySelectorAll(".cp-combobox-option"));
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
      empty.className = "cp-combobox-selected-empty";
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
        chip.className = "cp-combobox-chip";
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

if (!customElements.get("cp-combobox")) {
  customElements.define("cp-combobox", CpCombobox);
}
