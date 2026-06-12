use std::process::Command;
use std::fs;
use uuid::Uuid;

pub struct OcrResult {
    pub text: String,
}

/// Find the tesseract binary. Checks:
/// 1. TESSERACT_PATH env var (explicit override)
/// 2. Common install locations on macOS and Linux
/// 3. Falls back to bare "tesseract" (relies on PATH)
fn find_tesseract() -> String {
    // Allow explicit override
    if let Ok(path) = std::env::var("TESSERACT_PATH") {
        if !path.is_empty() {
            return path;
        }
    }

    let candidates = [
        // macOS Homebrew (Apple Silicon)
        "/opt/homebrew/bin/tesseract",
        // macOS Homebrew (Intel)
        "/usr/local/bin/tesseract",
        // Linux standard locations
        "/usr/bin/tesseract",
        "/usr/local/bin/tesseract",
        // Nix / NixOS
        "/run/current-system/sw/bin/tesseract",
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    // Fall back to PATH resolution
    "tesseract".to_string()
}

pub async fn extract_text(
    image_bytes: &[u8],
) -> Result<OcrResult, Box<dyn std::error::Error + Send + Sync>> {
    let preprocessed = preprocess_image(image_bytes)?;

    let id = Uuid::new_v4();
    let input_path = format!("/tmp/lv_input_{}.png", id);
    let output_base = format!("/tmp/lv_output_{}", id);
    let output_txt = format!("{}.txt", output_base);

    fs::write(&input_path, &preprocessed)?;

    // First pass: PSM 6 — assume uniform block of text
    let result = run_tesseract(&input_path, &output_base, "6").await;

    let text = if result.is_ok() {
        fs::read_to_string(&output_txt).unwrap_or_default()
    } else {
        // Fallback: PSM 3 — fully automatic page segmentation
        let fallback = run_tesseract(&input_path, &output_base, "3").await;
        if let Err(ref e) = fallback {
            // Give a human-readable hint about missing Tesseract
            let msg = e.to_string();
            if msg.contains("No such file") || msg.contains("not found") || msg.contains("os error 2") {
                let tess = find_tesseract();
                return Err(format!(
                    "Tesseract OCR binary not found (tried: {}). \
                    Install it with: macOS → `brew install tesseract`, \
                    Ubuntu/Debian → `sudo apt-get install tesseract-ocr`, \
                    Fedora → `sudo dnf install tesseract`. \
                    Or set the TESSERACT_PATH environment variable to the full path of the tesseract binary.",
                    tess
                ).into());
            }
        }
        fs::read_to_string(&output_txt).unwrap_or_default()
    };

    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&output_txt);

    Ok(OcrResult {
        text: text.trim().to_string(),
    })
}

async fn run_tesseract(
    input: &str,
    output_base: &str,
    psm: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tess_bin = find_tesseract();

    let mut cmd = Command::new(&tess_bin);
    cmd.arg(input)
        .arg(output_base)
        .arg("--oem")
        .arg("3") // LSTM neural net engine
        .arg("--psm")
        .arg(psm)
        .arg("-l")
        .arg("eng");

    // If TESSDATA_PREFIX is set in our env, forward it explicitly.
    // If not set, try common locations so local installs work without
    // requiring the user to set environment variables manually.
    if std::env::var("TESSDATA_PREFIX").is_err() {
        let candidates = [
            "/opt/homebrew/share/tessdata",         // macOS Homebrew Apple Silicon
            "/usr/local/share/tessdata",             // macOS Homebrew Intel / Linux
            "/usr/share/tesseract-ocr/5/tessdata",  // Ubuntu 22.04+
            "/usr/share/tesseract-ocr/4.00/tessdata", // Ubuntu 20.04
            "/usr/share/tessdata",                   // Fedora / older distros
        ];
        for dir in &candidates {
            if std::path::Path::new(dir).join("eng.traineddata").exists() {
                cmd.env("TESSDATA_PREFIX", dir);
                break;
            }
        }
    }

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "Tesseract binary not found at '{}'. \
                Install it with: macOS → `brew install tesseract`, \
                Ubuntu/Debian → `sudo apt-get install tesseract-ocr`, \
                Fedora → `sudo dnf install tesseract`. \
                Or set TESSERACT_PATH to the full path of the binary.",
                tess_bin
            )
        } else {
            format!("Failed to run tesseract: {}", e)
        }
    })?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(err.into());
    }
    Ok(())
}

fn preprocess_image(
    image_bytes: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    use image::{DynamicImage, ImageFormat, imageops};
    use std::io::Cursor;

    let img = image::load_from_memory(image_bytes)?;

    // Convert to grayscale for better OCR accuracy
    let gray = img.to_luma8();
    let (w, h) = (gray.width(), gray.height());

    // Upscale if image is small — Tesseract prefers ~300 DPI which for a typical
    // label shot works best above 1500px on the shorter side
    let min_dim = w.min(h);
    let processed = if min_dim < 1200 {
        let scale = 1400.0 / min_dim as f32;
        let new_w = (w as f32 * scale) as u32;
        let new_h = (h as f32 * scale) as u32;
        imageops::resize(&gray, new_w, new_h, imageops::FilterType::Lanczos3)
    } else {
        gray
    };

    // Contrast enhancement: stretch histogram
    let stretched = contrast_stretch(processed);

    let dyn_img = DynamicImage::ImageLuma8(stretched);
    let mut buf = Vec::new();
    dyn_img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)?;
    Ok(buf)
}

fn contrast_stretch(img: image::GrayImage) -> image::GrayImage {
    use image::Luma;

    let pixels: Vec<u8> = img.pixels().map(|p| p[0]).collect();

    let min_p = *pixels.iter().min().unwrap_or(&0);
    let max_p = *pixels.iter().max().unwrap_or(&255);

    if max_p == min_p {
        return img;
    }

    let range = (max_p - min_p) as f32;

    image::GrayImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y)[0];
        let stretched = ((p - min_p) as f32 / range * 255.0).round() as u8;
        Luma([stretched])
    })
}
