class CpBadge extends HTMLElement {
  static get observedAttributes() {
    return ["tone", "title"];
  }

  connectedCallback() {
    this.render();
  }

  attributeChangedCallback() {
    if (this.isConnected) {
      this.render();
    }
  }

  render() {
    const existing = this.querySelector("[data-cp-badge-inner]");
    let inner = existing;
    if (!(inner instanceof HTMLElement)) {
      const saved = this.innerHTML;
      this.innerHTML = "";
      inner = document.createElement("span");
      inner.setAttribute("data-cp-badge-inner", "true");
      inner.innerHTML = saved;
      this.appendChild(inner);
    }

    const tone = this.getAttribute("tone") || "state-neutral";
    inner.className = `cp-badge ${tone}`;
    const title = this.getAttribute("title");
    if (title === null) {
      inner.removeAttribute("title");
    } else {
      inner.setAttribute("title", title);
    }
  }
}

if (!customElements.get("cp-badge")) {
  customElements.define("cp-badge", CpBadge);
}
