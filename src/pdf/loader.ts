import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";
import { pdfjsLib } from "./worker";

export async function loadPdf(data: number[]): Promise<PDFDocumentProxy> {
	const pdfData = new Uint8Array(data);
	return await pdfjsLib.getDocument({ data: pdfData }).promise;
}
