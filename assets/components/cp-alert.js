class CpAlert extends HTMLElement {
  connectedCallback() {
    if (!this.hasAttribute("role")) {
      this.setAttribute("role", "status");
    }
  }
}

if (!customElements.get("cp-alert")) {
  customElements.define("cp-alert", CpAlert);
}
