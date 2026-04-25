import { invoke } from "@tauri-apps/api/core";
import { loadPdf } from "./pdf/loader";
import { renderPage } from "./pdf/renderer";
import { createViewerState } from "./pdf/state";
import { debounce } from "./pdf/debounce";
import { setupControls } from "./pdf/controls";
import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";

const canvas = document.querySelector("#slide-canvas") as HTMLCanvasElement;
const emptyState = document.querySelector("#empty-state") as HTMLDivElement;

let pdfDoc: PDFDocumentProxy | null = null;

const state = createViewerState(() => pdfDoc?.numPages ?? 0);

const render = debounce(async () => {
  if (pdfDoc) {
    await renderPage(pdfDoc, canvas, state.currentPage);
  }
}, 100);

async function openPdf() {
  const data = (await invoke("pick_pdf")) as number[] | null;
  if (!data) return;

  try {
    pdfDoc = await loadPdf(data);
    emptyState.style.display = "none";

    state.setPage(1);
    render();
  } catch (e) {
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
});

window.addEventListener("resize", render);
