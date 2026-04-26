import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";
import { createPageCache } from "./pdf/cache";
import { setupControls } from "./pdf/controls";
import { loadPdf } from "./pdf/loader";
import { getOrRenderPage, renderPage } from "./pdf/renderer";
import { createViewerState } from "./pdf/state";
import { debounce } from "./pdf/util";

const canvas = document.querySelector("#slide-canvas") as HTMLCanvasElement;
const emptyState = document.querySelector("#empty-state") as HTMLDivElement;

let pdfDoc: PDFDocumentProxy | null = null;
const cache = createPageCache();

const state = createViewerState(() => pdfDoc?.numPages ?? 0);

const render = debounce(async () => {
	if (pdfDoc) {
		const page = await pdfDoc.getPage(state.currentPage);
		const viewport = page.getViewport({ scale: 1 });
		const container = canvas.parentElement as HTMLElement;
		const scale = Math.min(
			container.clientWidth / viewport.width,
			container.clientHeight / viewport.height,
		);

		const dataUrl = await getOrRenderPage(pdfDoc, state.currentPage, cache, {
			scale,
		});
		await renderPage(canvas, dataUrl ?? "");
		preloadPages();
	}
}, 100);

async function preloadPages() {
	if (!pdfDoc) return;

	const page = await pdfDoc.getPage(state.currentPage);
	const viewport = page.getViewport({ scale: 1 });
	const container = canvas.parentElement as HTMLElement;
	const scale = Math.min(
		container.clientWidth / viewport.width,
		container.clientHeight / viewport.height,
	);

	const prevPage = state.currentPage - 1;
	const nextPage = state.currentPage + 1;

	if (prevPage >= 1) {
		getOrRenderPage(pdfDoc, prevPage, cache, { scale });
	}

	if (nextPage <= pdfDoc.numPages) {
		getOrRenderPage(pdfDoc, nextPage, cache, { scale });
	}
}

async function openPdf() {
	const data = (await invoke("pick_pdf")) as number[] | null;
	if (!data) return;

	try {
		pdfDoc = await loadPdf(data);
		emptyState.style.display = "none";
		cache.clear();

		state.setPage(1);
		render();
	} catch {
		emptyState.style.display = "flex";
	}
}

setupControls({
	next: () => {
		state.next();
		render();
	},
	prev: () => {
		state.prev();
		render();
	},
	open: openPdf,
	fullscreen: async () => {
		state.toggleFullscreen();
		await getCurrentWindow().setFullscreen(state.fullscreen);
	},
	quit: async () => {
		await getCurrentWindow().close();
	},
	goto: (page: number) => {
		state.setPage(page);
		render();
	},
});

window.addEventListener("resize", render);
