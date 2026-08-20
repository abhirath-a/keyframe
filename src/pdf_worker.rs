use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use egui::ColorImage;
use mupdf::{Colorspace, Document, Matrix};

/// Owns a background thread dedicated to rasterizing PDF pages.
///
/// MuPDF's `Document`/`Page` types are tied to a thread-local rendering
/// context, so we never move a `Document` between threads: the worker
/// thread opens its own `Document` once and keeps it for its whole
/// lifetime, receiving page indices to render over a channel.
pub struct PdfWorker {
    pub page_count: usize,
    request_tx: Sender<usize>,
    result_rx: Receiver<(usize, Arc<ColorImage>)>,
}

impl PdfWorker {
    /// Opens `path` (just to read the page count / fail fast on the calling
    /// thread) and spawns the render thread.
    pub fn open(path: &Path, dpi: f32) -> anyhow::Result<Self> {
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))?;

        // Open once here just to validate the file and get the page count
        // with a clean error on the caller's thread.
        let probe = Document::open(path_str)
            .map_err(|e| anyhow::anyhow!("failed to open '{}': {e}", path.display()))?;
        let page_count = probe.page_count()?.max(0) as usize;
        drop(probe);

        let (request_tx, request_rx) = mpsc::channel::<usize>();
        let (result_tx, result_rx) = mpsc::channel::<(usize, Arc<ColorImage>)>();

        let owned_path: PathBuf = path.to_path_buf();
        thread::Builder::new()
            .name("pdf-render".into())
            .spawn(move || render_loop(&owned_path, dpi, &request_rx, &result_tx))?;

        Ok(Self { page_count, request_tx, result_rx })
    }

    /// Ask the worker to render `page` (0-indexed). Non-blocking.
    pub fn request(&self, page: usize) {
        // If the receiver has hung up (thread panicked), just drop the
        // request; the app will keep showing a spinner for that page.
        let _ = self.request_tx.send(page);
    }

    /// Non-blocking poll for a finished render.
    pub fn try_recv(&self) -> Option<(usize, Arc<ColorImage>)> {
        self.result_rx.try_recv().ok()
    }
}

fn render_loop(
    path: &Path,
    dpi: f32,
    rx: &Receiver<usize>,
    tx: &Sender<(usize, Arc<ColorImage>)>,
) {
    let path_str = match path.to_str() {
        Some(s) => s,
        None => return,
    };
    let doc = match Document::open(path_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("render thread: failed to open document: {e}");
            return;
        }
    };

    let zoom = dpi / 72.0; // MuPDF's native unit is 72 dpi.
    let colorspace = Colorspace::device_rgb();

    while let Ok(page_number) = rx.recv() {
        match render_page(&doc, page_number, zoom, &colorspace) {
            Ok(image) => {
                if tx.send((page_number, Arc::new(image))).is_err() {
                    break; // app has shut down
                }
            }
            Err(e) => eprintln!("failed to render page {page_number}: {e}"),
        }
    }
}

fn render_page(
    doc: &Document,
    page_number: usize,
    zoom: f32,
    colorspace: &Colorspace,
) -> anyhow::Result<ColorImage> {
    let page = doc.load_page(page_number as i32)?;
    let matrix = Matrix::new_scale(zoom, zoom);
    // alpha = true so we get RGBA samples directly, matching egui::ColorImage.
    let pixmap = page.to_pixmap(&matrix, colorspace, true, false)?;
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    let image = ColorImage::from_rgba_unmultiplied([width, height], pixmap.samples());
    Ok(image)
}

/// Tracks which pages are cached, which are in-flight, and decides what to
/// prefetch / prune based on a sliding window around the current page.
pub struct PageCache {
    pub images: HashMap<usize, Arc<ColorImage>>,
    pending: HashSet<usize>,
}

impl PageCache {
    pub fn new() -> Self {
        Self { images: HashMap::new(), pending: HashSet::new() }
    }

    /// Pages to keep warm around `current`, closest first (so closest pages
    /// are requested - and therefore rendered - first).
    pub fn desired_window(current: usize, page_count: usize, radius: usize) -> Vec<usize> {
        let mut pages = vec![current];
        for d in 1..=radius {
            if current >= d {
                pages.push(current - d);
            }
            if current + d < page_count {
                pages.push(current + d);
            }
        }
        pages
    }

    /// Called once per frame: requests any missing pages in the window and
    /// evicts cached pages that fell outside it.
    pub fn refresh(&mut self, worker: &PdfWorker, current: usize, radius: usize) {
        let desired = Self::desired_window(current, worker.page_count, radius);

        for &page in &desired {
            if !self.images.contains_key(&page) && !self.pending.contains(&page) {
                worker.request(page);
                self.pending.insert(page);
            }
        }

        let keep: HashSet<usize> = desired.into_iter().collect();
        self.images.retain(|p, _| keep.contains(p));
    }

    /// Drains any pages the worker has finished rendering.
    /// Returns the pages that just became available, so the caller can
    /// upload them as GPU textures.
    pub fn drain_finished(&mut self, worker: &PdfWorker) -> Vec<(usize, Arc<ColorImage>)> {
        let mut finished = Vec::new();
        while let Some((page, image)) = worker.try_recv() {
            self.pending.remove(&page);
            self.images.insert(page, image.clone());
            finished.push((page, image));
        }
        finished
    }

    pub fn is_pending(&self, page: usize) -> bool {
        self.pending.contains(&page)
    }
}
