export function setupControls(actions: {
  next: () => void;
  prev: () => void;
  open: () => void;
}) {
  window.addEventListener("keydown", (event) => {
    switch (event.code) {
      case "Space":
      case "ArrowRight":
        event.preventDefault();
        actions.next();
        break;
      case "ArrowLeft":
        event.preventDefault();
        actions.prev();
        break;
      case "KeyO":
        event.preventDefault();
        actions.open();
        break;
    }
  });
}
