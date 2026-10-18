# Memory benchmark

Measured on Windows using the supplied `Slideshow template.pdf` (47,750 bytes),
at the default 150 DPI with the audience window open and presenter window closed.
The default window size and saved preferences were the same for both runs.
Each run used a fresh process, a four-second warmup, and three samples two
seconds apart. Values below are the median, in MiB.

- Original desktop executable: 191.62 working set, 89.20 private working set,
  118.33 private committed memory.
- Optimized release executable: 129.84 working set, 53.29 private working set,
  70.37 private committed memory.

That is a 32% reduction in total working set and a 40% reduction in private
working set and private commit. The total includes shared libraries and graphics
driver pages; private working set is the process's resident, unshared RAM.
Private commit includes allocations that may not be resident in physical RAM.
These measurements cover the first slide after loading, rather than a peak
across all slides. Other documents, screens, drivers, and presenter-window
configurations can produce different results.

The changes replace wgpu with the OpenGL renderer, open the PDF once on its
render thread, cap prefetched textures with a configurable 24 MiB budget, and
notify the UI when rendering finishes instead of polling it every 8 ms.
The current slide's rendering resolution is unchanged.

Reproduce with a release build and the same PDF:

```powershell
cargo build --release --locked
./scripts/measure-memory.ps1 -Binary ./target/release/keyframe.exe -Document ./deck.pdf
```

Validation: 23 regression tests passed, including byte-budget eviction, keeping
the current slide when it exceeds the budget, PDF rendering, pdfpc round trips,
and synchronized presenter navigation. Strict Clippy checks passed.
