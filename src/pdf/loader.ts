import { pdfjsLib } from "./worker";
import type { PDFDocumentProxy } from "pdfjs-dist/types/src/display/api";

export async function loadPdf(data: number[]): Promise<PDFDocumentProxy>  {
    const pdfData = new Uint8Array(data);
    return await pdfjsLib.getDocument({ data: pdfData }).promise;
}
