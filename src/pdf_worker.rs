use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use egui::ColorImage;
use mupdf::pdf::{PdfAnnotationType, PdfDocument};
use mupdf::{Colorspace, Document, Matrix};

/// Owns a background thread dedicated to rasterizing PDF pages.
///
/// MuPDF's `Document`/`Page` types are tied to a thread-local rendering
/// context, so we never move a `Document` between threads: the worker
/// thread opens its own `Document` once and keeps it for its whole
/// lifetime, receiving page indices to render over a channel.
pub struct PdfWorker {
    pub page_count: usize,
    pub labels: Vec<String>,
    pub native_notes: Vec<String>,
    raster_bytes: Vec<usize>,
    requests: Arc<(Mutex<RenderRequests>, Condvar)>,
    result_rx: Receiver<PageResult>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
struct RenderRequests {
    pages: VecDeque<usize>,
    active: Option<usize>,
    stopped: bool,
    repaint: Option<egui::Context>,
}

impl Drop for PdfWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.requests;
        {
            let mut requests = lock.lock().unwrap();
            requests.stopped = true;
            requests.pages.clear();
        }
        wake.notify_one();
        // Release a renderer blocked on the bounded result channel before joining.
        let (_, empty) = mpsc::channel();
        drop(std::mem::replace(&mut self.result_rx, empty));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type PageResult = (usize, Result<Arc<ColorImage>, String>);

struct PdfMetadata {
    labels: Vec<String>,
    native_notes: Vec<String>,
    raster_bytes: Vec<usize>,
}

fn raster_zoom(width: f32, height: f32, zoom: f32) -> f32 {
    zoom.min((16_000_000.0 / (width * height)).sqrt())
        .min(4096.0 / width.max(height))
}

fn dpi_zoom(dpi: f32) -> f32 {
    if dpi.is_finite() {
        dpi.clamp(72.0, 300.0) / 72.0
    } else {
        150.0 / 72.0
    }
}

fn read_metadata(doc: &PdfDocument, dpi: f32) -> anyhow::Result<PdfMetadata> {
    let count = doc.page_count()?.max(0) as usize;
    let mut metadata = PdfMetadata {
        labels: Vec::with_capacity(count),
        native_notes: Vec::with_capacity(count),
        raster_bytes: Vec::with_capacity(count),
    };
    for idx in 0..count {
        metadata.labels.push(
            doc.page_label(idx)
                .unwrap_or_else(|_| (idx + 1).to_string()),
        );
        let page = doc.load_pdf_page(idx as i32)?;
        let bounds = page.bounds()?;
        let zoom = raster_zoom(bounds.width(), bounds.height(), dpi_zoom(dpi));
        // Account for outward pixel rounding of a potentially translated page.
        let width = (bounds.width() * zoom).ceil().max(1.0) as usize + 1;
        let height = (bounds.height() * zoom).ceil().max(1.0) as usize + 1;
        metadata
            .raster_bytes
            .push(width.saturating_mul(height).saturating_mul(4));
        let mut text = Vec::new();
        for annotation in page.annotations() {
            if annotation.r#type()? != PdfAnnotationType::Text {
                continue;
            }
            if let Some(contents) = annotation.contents()? {
                if !contents.is_empty() {
                    text.push(contents.to_owned());
                }
            }
        }
        metadata.native_notes.push(text.join("\n\n"));
    }
    Ok(metadata)
}

fn open_native_document(path: &Path) -> anyhow::Result<Document> {
    #[cfg(windows)]
    let path = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("PDF path is not valid UTF-8"))?;
    Ok(Document::open(path)?)
}

impl PdfWorker {
    pub fn set_repaint_context(&self, ctx: &egui::Context) {
        let mut requests = self.requests.0.lock().unwrap();
        if requests.repaint.is_none() {
            requests.repaint = Some(ctx.clone());
        }
    }
    #[cfg(test)]
    pub fn test_stub(page_count: usize) -> Self {
        let (_, result_rx) = mpsc::sync_channel(2);
        Self {
            page_count,
            labels: (1..=page_count).map(|n| n.to_string()).collect(),
            native_notes: vec![String::new(); page_count],
            raster_bytes: vec![4; page_count],
            requests: Arc::new((Mutex::new(RenderRequests::default()), Condvar::new())),
            result_rx,
            thread: None,
        }
    }

    /// Open and inspect the PDF on its owning render thread. Only metadata
    /// crosses back to the UI; the same document is then used to render.
    pub fn open(path: &Path, dpi: f32) -> anyhow::Result<Self> {
        let requests = Arc::new((Mutex::new(RenderRequests::default()), Condvar::new()));
        let render_requests = requests.clone();
        let (result_tx, result_rx) = mpsc::sync_channel::<PageResult>(2);
        let (metadata_tx, metadata_rx) = mpsc::sync_channel(1);

        let owned_path: PathBuf = path.to_path_buf();
        let thread = thread::Builder::new()
            .name("pdf-render".into())
            .spawn(move || {
                let opened = open_native_document(&owned_path)
                    .and_then(|doc| Ok(PdfDocument::try_from(doc)?));
                match opened.and_then(|doc| Ok((read_metadata(&doc, dpi)?, doc))) {
                    Ok((metadata, doc)) => {
                        if metadata_tx.send(Ok(metadata)).is_ok() {
                            render_loop(&doc, dpi, &render_requests, &result_tx);
                        }
                    }
                    Err(error) => {
                        let _ = metadata_tx.send(Err(error.to_string()));
                    }
                }
            })?;
        let metadata = match metadata_rx.recv() {
            Ok(Ok(metadata)) => metadata,
            result => {
                let _ = thread.join();
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.to_string(),
                    _ => unreachable!(),
                };
                return Err(anyhow::anyhow!(
                    "failed to open '{}': {error}",
                    path.display()
                ));
            }
        };

        Ok(Self {
            page_count: metadata.labels.len(),
            labels: metadata.labels,
            native_notes: metadata.native_notes,
            raster_bytes: metadata.raster_bytes,
            requests,
            result_rx,
            thread: Some(thread),
        })
    }

    /// Ask the worker to render `page` (0-indexed). Non-blocking.
    fn set_requests(&self, pages: &[usize]) {
        let (lock, wake) = &*self.requests;
        let mut requests = lock.lock().unwrap();
        // Replace, rather than append: obsolete navigation never accumulates.
        requests.pages = pages
            .iter()
            .copied()
            .filter(|p| Some(*p) != requests.active)
            .collect();
        wake.notify_one();
    }

    /// Non-blocking poll for a finished render.
    pub fn try_recv(&self) -> Result<PageResult, TryRecvError> {
        self.result_rx.try_recv()
    }
}

fn render_loop(
    doc: &Document,
    dpi: f32,
    requests: &Arc<(Mutex<RenderRequests>, Condvar)>,
    tx: &SyncSender<PageResult>,
) {
    let zoom = dpi_zoom(dpi);
    let colorspace = Colorspace::device_rgb();

    loop {
        let page_number = {
            let (lock, wake) = &**requests;
            let mut state = lock.lock().unwrap();
            while state.pages.is_empty() && !state.stopped {
                state = wake.wait(state).unwrap();
            }
            if state.stopped {
                break;
            }
            let page = state.pages.pop_front().unwrap();
            state.active = Some(page);
            page
        };
        let result = render_page(doc, page_number, zoom, &colorspace)
            .map(Arc::new)
            .map_err(|e| e.to_string());
        if tx.send((page_number, result)).is_err() {
            break;
        }
        let repaint = {
            let mut state = requests.0.lock().unwrap();
            state.active = None;
            state.repaint.clone()
        };
        if let Some(ctx) = repaint {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
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
    let bounds = page.bounds()?;
    let width = bounds.width();
    let height = bounds.height();
    anyhow::ensure!(
        width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
        "Invalid slide dimensions"
    );
    // Cap an unusually large page at 16 megapixels (~64 MiB RGBA).
    let zoom = raster_zoom(width, height, zoom);
    let matrix = Matrix::new_scale(zoom, zoom);
    // Opaque RGB gives PDF pages their normal white background. Text note
    // annotations are kept in the presenter rather than painted on the slide.
    let pixmap = page
        .to_display_list(false)?
        .to_pixmap(&matrix, colorspace, false)?;
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    let image = ColorImage::from_rgb([width, height], pixmap.samples());
    Ok(image)
}

/// Tracks which pages are cached, which are in-flight, and decides what to
/// prefetch / prune based on a sliding window around the current page.
pub struct PageCache {
    pub images: HashMap<usize, Arc<ColorImage>>,
    pub errors: HashMap<usize, String>,
    pending: HashSet<usize>,
    desired: HashSet<usize>,
    uploaded: HashSet<usize>,
}

impl PageCache {
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            errors: HashMap::new(),
            pending: HashSet::new(),
            desired: HashSet::new(),
            uploaded: HashSet::new(),
        }
    }

    /// Pages to keep warm around `current`, closest first (so closest pages
    /// are requested - and therefore rendered - first).
    pub fn desired_window(current: usize, page_count: usize, radius: usize) -> Vec<usize> {
        if page_count == 0 {
            return Vec::new();
        }
        let current = current.min(page_count - 1);
        let mut pages = vec![current];
        for d in 1..=radius.min(5) {
            if current + d < page_count {
                pages.push(current + d);
            }
            if current >= d {
                pages.push(current - d);
            }
        }
        pages
    }

    /// Called once per frame: requests any missing pages in the window and
    /// evicts cached pages that fell outside it.
    #[cfg(test)]
    pub fn refresh(&mut self, worker: &PdfWorker, current: usize, radius: usize) {
        self.refresh_with_budget(worker, current, radius, 16 * 1024 * 1024);
    }

    pub fn refresh_with_budget(
        &mut self,
        worker: &PdfWorker,
        current: usize,
        radius: usize,
        budget: usize,
    ) {
        let desired = self.prepare_window(worker, current, radius, budget);
        let missing: Vec<_> = desired
            .into_iter()
            .filter(|p| {
                !self.images.contains_key(p)
                    && !self.uploaded.contains(p)
                    && !self.errors.contains_key(p)
            })
            .collect();
        self.pending = missing.iter().copied().collect();
        worker.set_requests(&missing);
    }

    pub fn prepare_window(
        &mut self,
        worker: &PdfWorker,
        current: usize,
        radius: usize,
        budget: usize,
    ) -> Vec<usize> {
        let mut used = 0usize;
        let desired: Vec<_> = Self::desired_window(current, worker.page_count, radius)
            .into_iter()
            .filter(|&page| {
                let bytes = worker.raster_bytes[page];
                if page == current.min(worker.page_count.saturating_sub(1))
                    || bytes <= budget.saturating_sub(used)
                {
                    used = used.saturating_add(bytes);
                    true
                } else {
                    false
                }
            })
            .collect();
        self.keep_window(&desired);
        desired
    }

    #[cfg(test)]
    pub fn set_window(&mut self, current: usize, count: usize, radius: usize) {
        self.keep_window(&Self::desired_window(current, count, radius));
    }

    fn keep_window(&mut self, desired: &[usize]) {
        self.desired = desired.iter().copied().collect();
        self.images.retain(|p, _| self.desired.contains(p));
        self.errors.retain(|p, _| self.desired.contains(p));
        self.pending.retain(|p| self.desired.contains(p));
        self.uploaded.retain(|p| self.desired.contains(p));
    }

    pub fn mark_uploaded(&mut self, page: usize) {
        self.images.remove(&page);
        self.uploaded.insert(page);
    }

    pub fn cached_pages(&self) -> HashSet<usize> {
        self.uploaded
            .iter()
            .chain(self.images.keys())
            .copied()
            .collect()
    }

    /// Drains any pages the worker has finished rendering.
    /// Returns the pages that just became available, so the caller can
    /// upload them as GPU textures.
    pub fn drain_finished(&mut self, worker: &PdfWorker) -> Vec<(usize, Arc<ColorImage>)> {
        let mut finished = Vec::new();
        loop {
            match worker.try_recv() {
                Ok((page, result)) => {
                    self.pending.remove(&page);
                    if !self.desired.contains(&page) {
                        continue;
                    }
                    match result {
                        Ok(image) => {
                            self.images.insert(page, image.clone());
                            finished.push((page, image));
                        }
                        Err(error) => {
                            self.errors.insert(page, error);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    for page in self.pending.drain() {
                        self.errors.insert(
                            page,
                            "The PDF renderer stopped. Reopen the document to retry.".into(),
                        );
                    }
                    break;
                }
            }
        }
        finished
    }

    #[cfg(test)]
    pub fn is_pending(&self, page: usize) -> bool {
        self.pending.contains(&page)
    }

    pub fn wants_page(&self, page: usize) -> bool {
        self.desired.contains(&page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn fake_worker(count: usize) -> (PdfWorker, SyncSender<PageResult>) {
        let (tx, result_rx) = mpsc::sync_channel(2);
        (
            PdfWorker {
                page_count: count,
                labels: (1..=count).map(|i| i.to_string()).collect(),
                native_notes: vec![String::new(); count],
                raster_bytes: vec![4; count],
                requests: Arc::new((Mutex::new(RenderRequests::default()), Condvar::new())),
                result_rx,
                thread: None,
            },
            tx,
        )
    }

    #[test]
    fn failed_render_clears_pending_and_can_be_retried_after_eviction() {
        let (worker, result_tx) = fake_worker(3);
        let mut cache = PageCache::new();
        cache.refresh(&worker, 0, 0);
        result_tx.send((0, Err("Damaged slide".into()))).unwrap();
        cache.drain_finished(&worker);
        assert!(!cache.is_pending(0));
        assert_eq!(cache.errors[&0], "Damaged slide");
        cache.refresh(&worker, 1, 0);
        assert!(!cache.errors.contains_key(&0));
        cache.refresh(&worker, 0, 0);
        assert!(cache.is_pending(0));
    }

    #[test]
    fn byte_budget_keeps_current_and_nearest_fitting_prefetch() {
        let (mut worker, _tx) = fake_worker(3);
        worker.raster_bytes = vec![10 * 1024 * 1024; 3];
        let mut cache = PageCache::new();
        assert_eq!(
            cache.prepare_window(&worker, 1, 1, 24 * 1024 * 1024),
            [1, 2]
        );
        cache.refresh_with_budget(&worker, 1, 1, 24 * 1024 * 1024);
        assert_eq!(worker.requests.0.lock().unwrap().pages, [1, 2]);
        assert!(!cache.wants_page(0));
    }

    #[test]
    fn current_slide_exceeding_budget_is_still_rendered_at_full_quality() {
        let (mut worker, _tx) = fake_worker(3);
        worker.raster_bytes = vec![40 * 1024 * 1024; 3];
        let mut cache = PageCache::new();
        assert_eq!(cache.prepare_window(&worker, 1, 1, 24 * 1024 * 1024), [1]);
        assert!(cache.wants_page(1));
    }

    #[test]
    fn shrinking_cache_budget_evicts_uploaded_prefetch() {
        let (mut worker, _tx) = fake_worker(3);
        worker.raster_bytes = vec![10 * 1024 * 1024; 3];
        let mut cache = PageCache::new();
        cache.prepare_window(&worker, 1, 1, 40 * 1024 * 1024);
        for page in 0..3 {
            cache.mark_uploaded(page);
        }
        cache.prepare_window(&worker, 1, 1, 10 * 1024 * 1024);
        assert_eq!(cache.cached_pages(), HashSet::from([1]));
    }

    #[test]
    fn invalid_pdf_reports_error_without_leaving_a_worker_running() {
        let dir = tempfile::tempdir().unwrap();
        assert!(PdfWorker::open(&dir.path().join("missing.pdf"), 150.0).is_err());
    }

    #[test]
    fn disconnected_renderer_resolves_pending_pages_to_errors() {
        let (worker, result_tx) = fake_worker(3);
        let mut cache = PageCache::new();
        cache.refresh(&worker, 1, 1);
        drop(result_tx);
        cache.drain_finished(&worker);
        assert!(cache.pending.is_empty());
        assert_eq!(cache.errors.len(), 3);
    }

    #[test]
    fn rapid_navigation_replaces_requests_and_discards_stale_results() {
        let (worker, tx) = fake_worker(10_000);
        let mut cache = PageCache::new();
        for page in 0..10_000 {
            cache.refresh(&worker, page, usize::MAX);
            assert!(worker.requests.0.lock().unwrap().pages.len() <= 11);
            assert!(cache.pending.len() <= 11);
        }
        tx.send((
            0,
            Ok(Arc::new(ColorImage::new([2, 2], egui::Color32::WHITE))),
        ))
        .unwrap();
        assert!(cache.drain_finished(&worker).is_empty());
        assert!(!cache.images.contains_key(&0));
    }

    #[test]
    fn cache_evicts_old_images_and_releases_their_last_reference() {
        let (worker, tx) = fake_worker(100);
        let mut cache = PageCache::new();
        cache.refresh(&worker, 0, 1);
        let image = Arc::new(ColorImage::new([2, 2], egui::Color32::WHITE));
        let weak = Arc::downgrade(&image);
        tx.send((0, Ok(image))).unwrap();
        drop(cache.drain_finished(&worker));
        assert!(weak.upgrade().is_some());
        cache.set_window(50, worker.page_count, 1);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn uploading_releases_cpu_pixels_without_requesting_a_second_render() {
        let (worker, tx) = fake_worker(2);
        let mut cache = PageCache::new();
        cache.refresh(&worker, 0, 0);
        let image = Arc::new(ColorImage::new([2, 2], egui::Color32::WHITE));
        let weak = Arc::downgrade(&image);
        tx.send((0, Ok(image))).unwrap();
        drop(cache.drain_finished(&worker));
        cache.mark_uploaded(0);
        assert!(weak.upgrade().is_none());
        cache.refresh(&worker, 0, 0);
        assert!(worker.requests.0.lock().unwrap().pages.is_empty());
        cache.refresh(&worker, 1, 0);
        assert!(!cache.cached_pages().contains(&0));
    }

    #[test]
    fn dropping_worker_unblocks_a_full_result_queue_and_joins_thread() {
        let (mut worker, tx) = fake_worker(3);
        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = completed.clone();
        worker.thread = Some(thread::spawn(move || {
            while tx.send((0, Err("test".into()))).is_ok() {}
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }));
        drop(worker);
        assert!(completed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn opens_and_renders_a_pdf_on_the_background_thread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("presentation.pdf");
        let content = "BT /F1 18 Tf 10 50 Td (Keyframe) Tj ET";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [6 0 R] >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
            "<< /Type /Annot /Subtype /Text /Rect [0 0 10 10] /P 3 0 R /Contents (Embedded speaker note) >>".to_owned(),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 7\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        std::fs::write(&path, pdf).unwrap();
        let worker = PdfWorker::open(&path, 72.0).unwrap();
        assert_eq!(worker.page_count, 1);
        assert_eq!(worker.native_notes, ["Embedded speaker note"]);
        let mut cache = PageCache::new();
        cache.refresh(&worker, 0, 1);
        let deadline = Instant::now() + Duration::from_secs(10);
        while cache.is_pending(0) && Instant::now() < deadline {
            cache.drain_finished(&worker);
            thread::sleep(Duration::from_millis(10));
        }
        assert!(cache.errors.is_empty());
        assert_eq!(cache.images[&0].size, [200, 100]);
        assert!(cache.images[&0].pixels.iter().any(|pixel| pixel.r() < 50));
        assert!(cache.images[&0].pixels.iter().all(|pixel| pixel.a() == 255));
    }
}
