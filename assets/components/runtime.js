export const STORAGE_KEYS = {
  theme: "ghprs.theme",
  filters: "ghprs.prFilters.v3",
};

const BATCH_NAV_COOLDOWN_MS = 450;
let lastNavigationAt = 0;

export function guardedNavigate(url) {
  const now = Date.now();
  if (now - lastNavigationAt < BATCH_NAV_COOLDOWN_MS) {
    return;
  }
  lastNavigationAt = now;
  window.location.assign(url);
}
