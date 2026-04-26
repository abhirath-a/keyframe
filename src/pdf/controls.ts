export function setupControls(actions: {
	next: () => void;
	prev: () => void;
	open: () => void;
	fullscreen: () => void;
	quit: () => void;
	goto: (page: number) => void;
}) {
	const keyActions: Record<string, () => void> = {
		Space: actions.next,
		ArrowRight: actions.next,
		ArrowLeft: actions.prev,
		KeyO: actions.open,
		KeyF: actions.fullscreen,
		F11: actions.fullscreen,
		KeyQ: actions.quit,
	};

	window.addEventListener("keydown", (event) => {
		// go to n-th slide when a number key + ctrl is pressed
		if (/^\d$/.test(event.key) && event.ctrlKey) {
			actions.goto(+event.key);
			return;
		}

		// look up action in keyActions map
		const action = keyActions[event.code];

		if (action) {
			event.preventDefault();
			action();
		}
	});

	// clicking on empty screen will open the file explorer
	document
		.getElementById("empty-state")
		?.addEventListener("mousedown", (event) => {
			event.preventDefault();
			actions.open();
		});

	// go to next slide on mouse press
	document
		.getElementById("slide-canvas")
		?.addEventListener("mousedown", (event) => {
			event.preventDefault();
			actions.next();
		});
}
