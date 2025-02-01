use actix_web::{
    middleware::Logger,
    web, App, HttpResponse, HttpServer,
    Result as ActixResult,
};
use image::{Rgba, RgbaImage, imageops::FilterType, ImageBuffer};
use qrc::{QRCode, qr_code_to};
use reqwest;
use std::io::Cursor;
use serde::Deserialize;
use moka::future::Cache;
use std::time::Duration;

// Cache key structure
#[derive(Hash, Eq, PartialEq, Clone)]
struct QRCacheKey {
    content: String,
    size: u32,
    fg_color: Option<String>,
    bg_color: Option<String>,
}

// Shared state structure
struct AppState {
    cache: Cache<QRCacheKey, Vec<u8>>,
}

#[derive(Deserialize)]
struct QRParams {
    content: String,
    size: Option<u32>,
    fg_color: Option<String>,
    bg_color: Option<String>,
    logo_url: Option<String>,
}

async fn fetch_and_resize_logo(url: &str, size: u32) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    let img = image::load_from_memory(&bytes)?;

    let logo_size = size / 4;
    let resized = img.resize(logo_size, logo_size, FilterType::Lanczos3);

    Ok(resized.to_rgba8())
}

fn hex_to_rgba(hex: &str) -> Result<Rgba<u8>, String> {
    if hex.len() != 7 || !hex.starts_with('#') {
        return Err("Invalid hex color format".to_string());
    }

    let r = u8::from_str_radix(&hex[1..3], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&hex[3..5], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&hex[5..7], 16).map_err(|e| e.to_string())?;

    Ok(Rgba([r, g, b, 255]))
}

fn calculate_safe_zone(qr_size: u32) -> (u32, u32, u32, u32) {
    // Calculate the center zone that's safe for logo placement
    // Typically, QR codes can have up to 30% error correction
    let safe_size = qr_size / 4;  // 25% of QR size
    let start_x = (qr_size - safe_size) / 2;
    let start_y = (qr_size - safe_size) / 2;

    (start_x, start_y, safe_size, safe_size)
}

async fn generate_qr_image(params: &QRParams, size: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Generate QR code with size
    let png = qr_code_to!(params.content.clone().into(), "png", size);
    let png_data = png.into_raw();

    // Create image buffer from raw data
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(size, size, png_data)
        .unwrap_or_else(|| RgbaImage::new(size, size));

    // Apply colors if provided
    if params.fg_color.is_some() || params.bg_color.is_some() {
        let fg_color = params.fg_color
            .as_ref()
            .and_then(|hex| hex_to_rgba(hex).ok())
            .unwrap_or(Rgba([0, 0, 0, 255]));

        let bg_color = params.bg_color
            .as_ref()
            .and_then(|hex| hex_to_rgba(hex).ok())
            .unwrap_or(Rgba([255, 255, 255, 255]));

        for pixel in image.pixels_mut() {
            *pixel = if pixel[0] == 0 { fg_color } else { bg_color };
        }
    }

    // Add logo if provided
    if let Some(logo_url) = &params.logo_url {
        if let Ok(mut logo) = fetch_and_resize_logo(logo_url, size).await {
            // Calculate safe zone for logo
            let (start_x, start_y, safe_width, safe_height) = calculate_safe_zone(size);

            // Resize logo to fit in safe zone
            logo = image::imageops::resize(&logo,
                safe_width,
                safe_height,
                FilterType::Lanczos3);

            // Create white background for logo
            let margin = 4; // pixels of white margin around logo
            for y in start_y.saturating_sub(margin)..start_y + safe_height + margin {
                for x in start_x.saturating_sub(margin)..start_x + safe_width + margin {
                    if x < size && y < size {
                        image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                    }
                }
            }

            // Overlay logo with transparency handling
            for (x, y, pixel) in logo.enumerate_pixels() {
                let target_x = start_x + x;
                let target_y = start_y + y;
                if target_x < size && target_y < size {
                    // Alpha blending
                    if pixel[3] > 0 {
                        let alpha = pixel[3] as f32 / 255.0;
                        let existing = image.get_pixel(target_x, target_y);
                        let blended = Rgba([
                            ((1.0 - alpha) * existing[0] as f32 + alpha * pixel[0] as f32) as u8,
                            ((1.0 - alpha) * existing[1] as f32 + alpha * pixel[1] as f32) as u8,
                            ((1.0 - alpha) * existing[2] as f32 + alpha * pixel[2] as f32) as u8,
                            255,
                        ]);
                        image.put_pixel(target_x, target_y, blended);
                    }
                }
            }
        }
    }

    // Convert to binary
    let mut buffer = Vec::new();
    image.write_to(&mut Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;

    Ok(buffer)
}

async fn generate_qr(
    params: web::Query<QRParams>,
    data: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let size = params.size.unwrap_or(512);

    // Create cache key
    let cache_key = QRCacheKey {
        content: params.content.clone(),
        size,
        fg_color: params.fg_color.clone(),
        bg_color: params.bg_color.clone(),
    };

    // Try to get from cache
    if let Some(cached_data) = data.cache.get(&cache_key).await {
        return Ok(HttpResponse::Ok()
            .content_type("image/png")
            .body(cached_data));
    }

    // Generate new QR code if not in cache
    let buffer = generate_qr_image(&params, size).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    // Store in cache
    data.cache.insert(cache_key, buffer.clone()).await;

    // Return response
    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(buffer))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Initialize cache
    let cache: Cache<QRCacheKey, Vec<u8>> = Cache::builder()
        .time_to_live(Duration::from_secs(3600)) // Cache for 1 hour
        .time_to_idle(Duration::from_secs(1800)) // Remove if not accessed for 30 minutes
        .max_capacity(1000) // Maximum number of items in cache
        .build();

    let app_state = web::Data::new(AppState { cache });

    println!("Server starting at http://127.0.0.1:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .route("/generate-qr", web::get().to(generate_qr))
            .route("/health", web::get().to(health_check))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

async fn health_check() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "healthy"})))
}
