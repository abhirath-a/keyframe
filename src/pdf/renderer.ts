import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";

let activeRenderTask: { cancel: () => void } | null = null;

export async function renderPage(
  pdfDoc: PDFDocumentProxy,
  canvas: HTMLCanvasElement,
  pageNumber: number
) {
  if (activeRenderTask) {
    activeRenderTask.cancel();
    activeRenderTask = null;
  }

  const page = await pdfDoc.getPage(pageNumber);
  const container = canvas.parentElement as HTMLElement;

  const dpr = window.devicePixelRatio || 1;
  const viewport = page.getViewport({ scale: 1 });

  const scale = Math.min(
    container.clientWidth / viewport.width,
    container.clientHeight / viewport.height
  );

  const scaledViewport = page.getViewport({ scale });

  const context = canvas.getContext("2d", { alpha: false });
  if (!context) return;

  canvas.width = Math.floor(scaledViewport.width * dpr);
  canvas.height = Math.floor(scaledViewport.height * dpr);

  canvas.style.width = `${Math.floor(scaledViewport.width)}px`;
  canvas.style.height = `${Math.floor(scaledViewport.height)}px`;

  context.setTransform(dpr, 0, 0, dpr, 0, 0);
  context.clearRect(0, 0, canvas.width, canvas.height);

  const renderTask = page.render({
    canvas,
    canvasContext: context,
    viewport: scaledViewport,
    intent: "display",
  });

  activeRenderTask = renderTask;

  await renderTask.promise;
  activeRenderTask = null;
}
