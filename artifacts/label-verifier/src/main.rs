use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{middleware::Logger, web, App, Error, HttpResponse, HttpServer};
use futures_util::TryStreamExt;
use std::env;
use std::path::PathBuf;

mod models;
mod ocr;
mod render;
mod verify;

use models::{ApplicationData, VerificationStatus};

/// Return the directory where static files live.
/// Defaults to "static" (relative to CWD, correct for `cargo run` from the crate root).
/// Set STATIC_DIR to an absolute path when running the release binary from a different
/// working directory (e.g. in production deployments).
fn static_dir() -> PathBuf {
    if let Ok(dir) = env::var("STATIC_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    PathBuf::from("static")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let sdir = static_dir();
    log::info!("Label Verifier running on http://0.0.0.0:{}", port);
    log::info!("Serving static files from: {}", sdir.display());

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::new("%r → %s (%Dms)"))
            .route("/", web::get().to(index))
            .route("/verify", web::post().to(verify_single))
            .route("/verify/batch", web::post().to(verify_batch))
            .service(actix_files::Files::new("/static", sdir.clone()))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

async fn index() -> Result<NamedFile, Error> {
    Ok(NamedFile::open(static_dir().join("index.html"))?)
}

// ---------------------------------------------------------------------------
// Single label verification
// ---------------------------------------------------------------------------

async fn verify_single(mut payload: Multipart) -> Result<HttpResponse, Error> {
    let mut image_bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut app_data = ApplicationData::default();

    while let Some(mut field) = payload.try_next().await? {
        let name = field.name().unwrap_or("").to_string();
        let cd_filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .map(|s| s.to_string());

        let mut data: Vec<u8> = Vec::new();
        while let Some(chunk) = field.try_next().await? {
            data.extend_from_slice(&chunk);
        }

        match name.as_str() {
            "image" => {
                filename = cd_filename;
                if !data.is_empty() {
                    image_bytes = Some(data);
                }
            }
            "brand_name" => set_opt(&mut app_data.brand_name, &data),
            "class_type" => set_opt(&mut app_data.class_type, &data),
            "abv" => set_opt(&mut app_data.abv, &data),
            "net_contents" => set_opt(&mut app_data.net_contents, &data),
            "producer_name" => set_opt(&mut app_data.producer_name, &data),
            "country_of_origin" => set_opt(&mut app_data.country_of_origin, &data),
            "check_government_warning" => {
                let v = String::from_utf8_lossy(&data);
                app_data.check_government_warning = v == "on" || v == "true" || v == "1";
            }
            _ => {}
        }
    }

    let Some(image) = image_bytes else {
        return Ok(render::error_fragment("No image file received. Please attach a label image."));
    };

    let ocr_result = match ocr::extract_text(&image).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("OCR error: {}", e);
            return Ok(render::error_fragment(&format!(
                "OCR processing failed: {}. Please try a clearer image.",
                e
            )));
        }
    };

    let result = verify::verify_label(&ocr_result.text, &app_data, filename);
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render::single_result_html(&result)))
}

// ---------------------------------------------------------------------------
// Batch verification
// ---------------------------------------------------------------------------

async fn verify_batch(mut payload: Multipart) -> Result<HttpResponse, Error> {
    let mut images: Vec<(Vec<u8>, String)> = Vec::new();
    let mut app_data = ApplicationData::default();

    while let Some(mut field) = payload.try_next().await? {
        let name = field.name().unwrap_or("").to_string();
        let fname = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .unwrap_or("unknown")
            .to_string();

        let mut data: Vec<u8> = Vec::new();
        while let Some(chunk) = field.try_next().await? {
            data.extend_from_slice(&chunk);
        }

        match name.as_str() {
            "images" => {
                if !data.is_empty() {
                    images.push((data, fname));
                }
            }
            "brand_name" => set_opt(&mut app_data.brand_name, &data),
            "class_type" => set_opt(&mut app_data.class_type, &data),
            "abv" => set_opt(&mut app_data.abv, &data),
            "net_contents" => set_opt(&mut app_data.net_contents, &data),
            "producer_name" => set_opt(&mut app_data.producer_name, &data),
            "country_of_origin" => set_opt(&mut app_data.country_of_origin, &data),
            "check_government_warning" => {
                let v = String::from_utf8_lossy(&data);
                app_data.check_government_warning = v == "on" || v == "true" || v == "1";
            }
            _ => {}
        }
    }

    if images.is_empty() {
        return Ok(render::error_fragment("No image files received."));
    }

    let mut results = Vec::new();

    for (img_bytes, fname) in images {
        let ocr_result = match ocr::extract_text(&img_bytes).await {
            Ok(r) => r,
            Err(e) => {
                log::error!("OCR error for {}: {}", fname, e);
                results.push(models::VerificationResult {
                    fields: vec![],
                    overall_status: VerificationStatus::Fail,
                    ocr_text: String::new(),
                    summary: format!("OCR failed: {}", e),
                    filename: Some(fname),
                });
                continue;
            }
        };

        let result = verify::verify_label(&ocr_result.text, &app_data, Some(fname));
        results.push(result);
    }

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render::batch_results_html(&results)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_opt(target: &mut Option<String>, data: &[u8]) {
    let s = String::from_utf8_lossy(data).trim().to_string();
    if !s.is_empty() {
        *target = Some(s);
    }
}
