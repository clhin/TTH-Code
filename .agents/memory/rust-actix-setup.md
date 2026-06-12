---
name: Rust Actix Web in pnpm workspace
description: How to run a Rust Actix Web app in this pnpm monorepo workspace and quirks encountered.
---

## Artifact Registration
- `createArtifact` does not support a Rust artifact type.
- `verifyAndReplaceArtifactToml` cannot create new artifacts from scratch (errors on missing/empty artifact.toml).
- Workaround: configure a `webview` workflow directly on a supported port (e.g. 3000). The app is accessible via the Replit preview system even without a formal artifact registration.
- **Why:** The Replit artifact system is designed for its own scaffold types; Rust is outside that set.

## actix-multipart v0.7 API
- `field.name()` returns `Option<&str>` — call `.unwrap_or("")` before `.to_string()`.
- `field.content_disposition()` returns `Option<&ContentDisposition>` — use `.and_then(|cd| cd.get_filename())`.
- **Why:** The v0.7 API changed from v0.6 which returned bare references.

## Tesseract in Nix
- Install via `installSystemDependencies(["tesseract", "leptonica", "pkg-config", "clang"])`.
- The Nix tesseract package puts the binary on PATH automatically; no `TESSDATA_PREFIX` needed.
- Best invocation: `tesseract <input> <output_base> --oem 3 --psm 6 -l eng` (LSTM engine, uniform block).
- Fallback to PSM 3 (fully automatic) if PSM 6 fails.

## image crate v0.25
- Use `DynamicImage::write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)` to encode to bytes.
- `imageops::resize` takes a `FilterType`, e.g. `imageops::FilterType::Lanczos3`.
