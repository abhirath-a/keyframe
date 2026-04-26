import { clamp } from "./util";

export function createViewerState(getPageCount: () => number) {
	let currentPage = 1;
	let fullscreen = true;

	function setPage(page: number) {
		const max = getPageCount();
		currentPage = clamp(page, 1, max);
	}

	return {
		get currentPage() {
			return currentPage;
		},
		get fullscreen() {
			return fullscreen;
		},
		toggleFullscreen() {
			fullscreen = !fullscreen;
		},

		setPage,

		next() {
			setPage(currentPage + 1);
		},

		prev() {
			setPage(currentPage - 1);
		},
	};
}
