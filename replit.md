# TTB Label Verification Tool

An AI-powered alcohol beverage label compliance review tool for TTB (Alcohol and Tobacco Tax and Trade Bureau) agents. Upload a label photo and enter application data — the tool OCRs the label and does a field-by-field compliance check.

## Run & Operate

- **Start the app:** workflow `Start application` → `cargo run` on port 3000
- **Stack:** Rust 1.x + Actix Web 4 + Tesseract 5 OCR + HTMX + Tailwind CSS
- **Rebuild after code changes:** `cd artifacts/label-verifier && cargo build`

## Local Setup (Linux)

### Prerequisites

You need three things installed:
1. **Rust** (stable, 1.70+)
2. **Tesseract OCR** (version 5.x)
3. **Git** (to clone the repo)

### Linux (Ubuntu/Debian)

1. **Install Tesseract and system dependencies**:
   ```bash
   sudo apt-get update
   sudo apt-get install -y tesseract-ocr libtesseract-dev libleptonica-dev pkg-config
   ```
   Verify:
   ```bash
   tesseract --version
   ```

2. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```

3. **Clone and build**:
   ```bash
   git clone <repo-url>
   cd <repo>/artifacts/label-verifier
   cargo build --release
   ```

4. **Run**:
   ```bash
   PORT=3000 cargo run --release
   ```

### Linux (Fedora/RHEL/CentOS)

1. **Install Tesseract**:
   ```bash
   sudo dnf install tesseract tesseract-langpack-eng leptonica-devel
   ```

2. **Install Rust** via rustup (same as above), then build and run.

### Troubleshooting

| Issue | Fix |
|---|---|
| `tesseract: command not found` | Tesseract is not on PATH. Install it (see above) or set `TESSDATA_PREFIX` to the directory containing `eng.traineddata`. |
| `failed to run custom build command` for `leptonica` | On Linux, install `libtesseract-dev` (Debian) or `tesseract-devel` (Fedora). |
| `linker cc not found`  `sudo apt install build-essential` (Linux). |
| First build is very slow | Normal — Cargo is downloading and compiling all dependencies. Subsequent builds are fast. |
| Batch uploads hang | The app processes images sequentially. For 100+ images, consider increasing the request timeout or running the server with `tokio` runtime tuning. |

## Stack

- **Backend:** Rust + Actix Web 4 (single binary, no Node.js involved)
- **OCR:** Tesseract 5 (LSTM engine, open source) — called via CLI subprocess
- **Frontend:** HTMX 2 + Tailwind CSS CDN — no build step for the UI
- **Image preprocessing:** Rust `image` crate (grayscale + contrast stretch + upscale)
- **Fuzzy matching:** `strsim` crate (normalized Levenshtein) for brand/class/producer fields
- **Numeric matching:** Regex extraction + tolerance bands for ABV and net contents

## Where Things Live

- `artifacts/label-verifier/` — the entire app (Rust crate)
  - `src/main.rs` — Actix Web server, route handlers
  - `src/ocr.rs` — Tesseract integration + image preprocessing
  - `src/verify.rs` — field-by-field verification logic (all TTB fields)
  - `src/render.rs` — server-side HTML fragment renderer (HTMX responses)
  - `src/models.rs` — shared types
  - `static/index.html` — HTMX + Tailwind UI (single/batch modes)

## Architecture Decisions

- **Tesseract CLI subprocess (not crate bindings):** Avoids complex native library linking in the Nix environment. Slightly higher per-image overhead but far simpler dependency graph and more reliable builds.
- **Server-rendered HTML fragments:** HTMX receives raw HTML from the server, keeping JS to near-zero. No client-side framework, no bundler.
- **Fuzzy matching with explicit thresholds:** ≥88% similarity = Pass, 65–88% = Warning (manual review), <65% = Fail. Handles Dave's STONE'S THROW / Stone's Throw case naturally.
- **Government warning exact-first, fuzzy fallback:** Exact normalized match first; ≥92% Levenshtein as OCR-noise fallback; ≥75% = Warning. Enforces ALL-CAPS prefix check separately per 27 CFR § 16.21.
- **Image preprocessing pipeline:** Grayscale → histogram contrast stretch → upscale to ≥1400px short edge. Handles poor lighting, glare, and low-resolution shots before Tesseract sees them.

## Product

- **Single label mode:** Upload one image + enter application fields → instant field-by-field result table (Pass / Warning / Fail badges) with extracted text disclosure
- **Batch mode:** Upload 200+ images → expandable per-label summary table with pass/fail counts
- **Government warning check:** Validates ALL CAPS header + word-for-word body text per 27 CFR § 16.21
- **Target latency:** <5 seconds per label (Tesseract 5 LSTM on a preprocessed image)

## User Preferences

- Actix Web (Rust) for the backend — explicitly requested

## Gotchas

- Rust first compile takes ~60–90 seconds (all deps from scratch); subsequent rebuilds are seconds
- Tesseract requires `TESSDATA_PREFIX` or the binary must be on `PATH` — Nix puts it on PATH automatically
- `image` crate v0.25 API: use `DynamicImage::write_to(&mut Cursor, ImageFormat::Png)` not `.save()`
- actix-multipart v0.7: `field.name()` returns `Option<&str>`, `content_disposition()` returns `Option<&ContentDisposition>` — both need explicit unwrap/and_then
- `verifyAndReplaceArtifactToml` cannot create brand-new artifacts; only Replit-supported artifact types work with `createArtifact`. The label-verifier runs as a plain webview workflow on port 3000.

## Pointers

- See the `pnpm-workspace` skill for workspace structure, TypeScript setup, and package details
