import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";
import type { createPageCache } from "./cache";

let activeRenderTask: { cancel: () => void } | null = null;

export async function renderPage(canvas: HTMLCanvasElement, dataUrl: string) {
	if (activeRenderTask) {
		activeRenderTask.cancel();
		activeRenderTask = null;
	}

	const container = canvas.parentElement as HTMLElement;
	const dpr = window.devicePixelRatio || 1;

	const image = new Image();
	image.src = dataUrl;
	await image.decode();

	const scale = Math.min(
		container.clientWidth / image.width,
		container.clientHeight / image.height,
	);

	const scaledWidth = image.width * scale;
	const scaledHeight = image.height * scale;

	const context = canvas.getContext("2d", { alpha: false });
	if (!context) return;

	canvas.width = Math.floor(scaledWidth * dpr);
	canvas.height = Math.floor(scaledHeight * dpr);

	canvas.style.width = `${Math.floor(scaledWidth)}px`;
	canvas.style.height = `${Math.floor(scaledHeight)}px`;

	context.setTransform(dpr, 0, 0, dpr, 0, 0);
	context.clearRect(0, 0, canvas.width, canvas.height);
	context.drawImage(image, 0, 0, scaledWidth, scaledHeight);
}

export async function getOrRenderPage(
	pdfDoc: PDFDocumentProxy,
	pageNumber: number,
	cache: ReturnType<typeof createPageCache>,
	options: { scale: number },
) {
	const { scale } = options;
	const key = `${pageNumber}-${scale.toPrecision(3)}`;

	if (cache.has(key)) {
		return cache.get(key);
	}

	const page = await pdfDoc.getPage(pageNumber);
	const viewport = page.getViewport({ scale });
	const dpr = window.devicePixelRatio || 1;

	const canvas = document.createElement("canvas");
	const context = canvas.getContext("2d", { alpha: false });
	if (!context) throw new Error("Could not get canvas context");

	canvas.width = Math.floor(viewport.width * dpr);
	canvas.height = Math.floor(viewport.height * dpr);
	canvas.style.width = `${Math.floor(viewport.width)}px`;
	canvas.style.height = `${Math.floor(viewport.height)}px`;

	context.scale(dpr, dpr);

	const renderTask = page.render({
    canvas ,
		canvasContext: context,
		viewport: viewport,
		intent: "display",
	});

	await renderTask.promise;
	const dataUrl = canvas.toDataURL("image/png");
	cache.set(key, dataUrl);
	return dataUrl;
}
