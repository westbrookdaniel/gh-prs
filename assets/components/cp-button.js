function copyAttr(host, target, name) {
  const value = host.getAttribute(name);
  if (value === null) {
    target.removeAttribute(name);
    return;
  }
  target.setAttribute(name, value);
}

function copyBooleanAttr(host, target, name) {
  if (host.hasAttribute(name)) {
    target.setAttribute(name, "");
  } else {
    target.removeAttribute(name);
  }
}

class CpButton extends HTMLElement {
  static get observedAttributes() {
    return [
      "variant",
      "size",
      "type",
      "name",
      "value",
      "form",
      "formaction",
      "formenctype",
      "formmethod",
      "formtarget",
      "formnovalidate",
      "disabled",
      "href",
      "target",
      "rel",
      "download",
      "title",
      "aria-label",
    ];
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
    const asLink = this.hasAttribute("href");
    const existing = this.querySelector("[data-cp-button-inner]");
    const expectedTag = asLink ? "A" : "BUTTON";

    let inner = existing;
    if (!(inner instanceof HTMLElement) || inner.tagName !== expectedTag) {
      const saved = inner instanceof HTMLElement ? inner.innerHTML : this.innerHTML;
      this.innerHTML = "";
      inner = document.createElement(asLink ? "a" : "button");
      inner.setAttribute("data-cp-button-inner", "true");
      inner.innerHTML = saved;
      this.appendChild(inner);
    }

    const variant = this.getAttribute("variant") || "default";
    const size = this.getAttribute("size") || "md";
    inner.className = `cp-button cp-button--${variant} cp-button--${size}`;

    if (asLink) {
      const disabled = this.hasAttribute("disabled");
      if (disabled) {
        inner.removeAttribute("href");
        inner.setAttribute("aria-disabled", "true");
        inner.tabIndex = -1;
      } else {
        copyAttr(this, inner, "href");
        inner.removeAttribute("aria-disabled");
        inner.removeAttribute("tabindex");
      }
      copyAttr(this, inner, "target");
      copyAttr(this, inner, "rel");
      copyAttr(this, inner, "download");
      copyAttr(this, inner, "title");
      copyAttr(this, inner, "aria-label");
    } else {
      inner.setAttribute("type", this.getAttribute("type") || "button");
      copyAttr(this, inner, "name");
      copyAttr(this, inner, "value");
      copyAttr(this, inner, "form");
      copyAttr(this, inner, "formaction");
      copyAttr(this, inner, "formenctype");
      copyAttr(this, inner, "formmethod");
      copyAttr(this, inner, "formtarget");
      copyAttr(this, inner, "title");
      copyAttr(this, inner, "aria-label");
      copyBooleanAttr(this, inner, "formnovalidate");
      copyBooleanAttr(this, inner, "disabled");
    }
  }
}

if (!customElements.get("cp-button")) {
  customElements.define("cp-button", CpButton);
}
