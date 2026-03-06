class CpCard extends HTMLElement {}

if (!customElements.get("cp-card")) {
  customElements.define("cp-card", CpCard);
}
