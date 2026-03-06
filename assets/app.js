import "/assets/components/index.js";

document.addEventListener(
  "error",
  (event) => {
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
  },
  true,
);
