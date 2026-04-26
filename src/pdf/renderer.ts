import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";
import type { createPageCache } from "./cache";

let activeRenderTask: { cancel: () => void } | null = null;

export async function renderPage(
	canvas: HTMLCanvasElement,
	bitmap: ImageBitmap,
) {
	if (activeRenderTask) {
		activeRenderTask.cancel();
		activeRenderTask = null;
	}

	const container = canvas.parentElement as HTMLElement;
	const dpr = window.devicePixelRatio || 1;

	const cssW = bitmap.width / dpr;
	const cssH = bitmap.height / dpr;

	const scale = Math.min(
		container.clientWidth / cssW,
		container.clientHeight / cssH,
	);

	const scaledCssW = cssW * scale;
	const scaledCssH = cssH * scale;

	const context = canvas.getContext("2d", { alpha: false });
	if (!context) return;

	canvas.width = Math.round(scaledCssW * dpr);
	canvas.height = Math.round(scaledCssH * dpr);
	canvas.style.width = `${Math.round(scaledCssW)}px`;
	canvas.style.height = `${Math.round(scaledCssH)}px`;

	context.setTransform(1, 0, 0, 1, 0, 0);
	context.clearRect(0, 0, canvas.width, canvas.height);

	context.imageSmoothingEnabled = false;

	context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
}

export async function getOrRenderPage(
	pdfDoc: PDFDocumentProxy,
	pageNumber: number,
	cache: ReturnType<typeof createPageCache>,
	options: { scale: number },
): Promise<ImageBitmap> {
	const { scale } = options;
	const key = `${pageNumber}-${scale.toPrecision(3)}`;

	const cached = cache.get(key);
	if (cached) return cached;

	const page = await pdfDoc.getPage(pageNumber);
	const viewport = page.getViewport({ scale });
	const dpr = window.devicePixelRatio || 1;

	const canvas = document.createElement("canvas");
	const context = canvas.getContext("2d", { alpha: false });
	if (!context) throw new Error("Could not get canvas context");

	canvas.width = Math.round(viewport.width * dpr);
	canvas.height = Math.round(viewport.height * dpr);

	context.setTransform(dpr, 0, 0, dpr, 0, 0);

	const renderTask = page.render({
		canvas,
		canvasContext: context,
		viewport,
		intent: "display",
	});

	activeRenderTask = { cancel: () => renderTask.cancel() };

	await renderTask.promise;

	const bitmap = await createImageBitmap(canvas);
	cache.set(key, bitmap);
	return bitmap;
}
