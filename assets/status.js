// @ts-check

/** @typedef {{ message: string }} GlobalStatus */
/** @typedef {{ container: HTMLElement | null, text: HTMLElement | null }} GlobalStatusElements */

/** @type {Map<string, GlobalStatus>} */
const globalStatuses = new Map();

/**
 * @returns {GlobalStatusElements}
 */
function globalStatusElements() {
  const container = document.querySelector("[data-global-status]");
  const text = document.querySelector("[data-global-status-text]");

  return {
    container: container instanceof HTMLElement ? container : null,
    text: text instanceof HTMLElement ? text : null,
  };
}

function renderGlobalStatus() {
  const { container, text } = globalStatusElements();
  if (!(container instanceof HTMLElement) || !(text instanceof HTMLElement)) {
    return;
  }

  if (globalStatuses.size === 0) {
    container.hidden = true;
    text.textContent = "";
    return;
  }

  const lastStatus = Array.from(globalStatuses.values()).at(-1);
  container.hidden = false;
  text.textContent = lastStatus ? lastStatus.message : "Working...";
}

/**
 * @param {string} key
 * @param {string} message
 */
export function addGlobalStatus(key, message) {
  if (typeof key !== "string" || key === "") {
    return;
  }

  globalStatuses.delete(key);
  globalStatuses.set(key, {
    message:
      typeof message === "string" && message !== "" ? message : "Working...",
  });
  renderGlobalStatus();
}

/**
 * @param {string} key
 */
export function removeGlobalStatus(key) {
  if (typeof key !== "string" || key === "") {
    return;
  }

  globalStatuses.delete(key);
  renderGlobalStatus();
}
