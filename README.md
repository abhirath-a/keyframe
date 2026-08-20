# Keyframe

A minimal, cross-platform PDF slideshow presenter. 

## Build prerequisites

`mupdf-sys` compiles MuPDF from source at build time (there's no
lightweight "just bind to a system lib" option unless you enable the
`sys-lib` Cargo feature and have MuPDF already installed). You'll need:

- A C/C++ compiler toolchain
  - Linux: `build-essential` (gcc/g++, make) or clang
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Windows: MSVC Build Tools (Visual Studio "Desktop development with
    C++" workload)
- `cmake`
- `python3` (used by MuPDF's build scripts)
- A recent Rust stable toolchain


## Build & run

```sh
cargo run --release
```

## Controls

Both arrow-key/"normie" bindings and vim-style bindings are active at
the same time by default:

| Action              | Default keys                              |
|---------------------|--------------------------------------------|
| Next slide          | `→` `↓` `Space` `PageDown` `j` `l` `n`      |
| Previous slide       | `←` `↑` `Backspace` `PageUp` `k` `h` `p`    |
| First slide          | `Home` `g`                                  |
| Last slide           | `End` `Shift+G`                             |
| Toggle fullscreen    | `f` `F11`                                   |
| Quit                 | `q` `Esc`                                   |

## Configuration

- Linux: `~/.config/keyframe/config.toml`
- macOS: `~/Library/Application Support/dev.keyframe.keyframe/config.toml`
- Windows: `%APPDATA%\keyframe\keyframe\config\config.toml`

```toml
# Pages to prefetch/cache on each side of the current page.
# 1 = current + immediate previous/next (the default).
cache_window = 1

# Rendering resolution in DPI. Higher = crisper on big/high-DPI
# displays, slower to render, more memory per cached page.
render_dpi = 150.0

# Background/letterbox color, [r, g, b] 0-255.
background_color = [18, 18, 18]

start_fullscreen = false
vsync = true

# Optional keybind overrides, leave out an action to keep its defaults.
[keybinds]
next_page = ["Right", "Space", "j", "l"]
prev_page = ["Left", "Backspace", "k", "h"]
```
