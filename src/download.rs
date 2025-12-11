use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::conversion::convert_apng_to_gif;
use crate::metadata::Sticker;
use crate::utils::save_image;

use reqwest;
use zip::ZipArchive;

/// Return true when the given pack id looks like an emoji pack id.
pub fn is_emoji_pack(pack_id: &str) -> bool {
    pack_id.len() == 24 && pack_id.chars().all(|c| c.is_ascii_hexdigit())
}

/// Checks if there are any animated stickers in the pack.
/// For emoji packs (hex 24 ids) this always returns false because emojis
/// are handled via separate emoji ZIPs.
pub async fn check_for_animated_stickers(pack_id: &str, stickers: &[Sticker]) -> bool {
    if is_emoji_pack(pack_id) {
        return false;
    }

    for sticker in stickers {
        let url = format!(
            "https://sdl-stickershop.line.naver.jp/products/0/0/1/{}/iphone/animation/{}@2x.png",
            pack_id,
            sticker.id
        );
        if let Ok(response) = reqwest::get(&url).await {
            if response.status().is_success() {
                return true;
            }
        }
    }
    false // No animated stickers found
}

/// Downloads stickers based on the selected file type (png, gif, or both).
/// This function ist nur für Sticker-Packs gedacht.
/// Emoji-Packs werden separat behandelt.
pub async fn download_stickers(
    pack_id: &str,
    stickers: &[Sticker],
    pack_name: &str,
    pack_ext: &str,
) {
    match pack_ext {
        "both" => {
            download_static_stickers(stickers, pack_name).await;
            download_animated_stickers(pack_id, stickers, pack_name).await;
        }
        "png" => download_static_stickers(stickers, pack_name).await,
        "gif" => download_animated_stickers(pack_id, stickers, pack_name).await,
        _ => eprintln!("Invalid file type provided."),
    }
}

/// Downloads static PNG stickers (used for sticker packs).
pub async fn download_static_stickers(stickers: &[Sticker], pack_name: &str) {
    for sticker in stickers {
        // Standard-Endpoint für statische Sticker-PNGs
        let url = format!(
            "http://dl.stickershop.line.naver.jp/stickershop/v1/sticker/{}/iphone/sticker@2x.png",
            sticker.id
        );
        let path = PathBuf::from(format!("{}/{}.png", pack_name, sticker.id));
        save_image(&url, &path).await;
    }
}

/// Downloads animated stickers (converts APNG to GIF).
async fn download_animated_stickers(pack_id: &str, stickers: &[Sticker], pack_name: &str) {
    for sticker in stickers {
        let apng_url = format!(
            "https://sdl-stickershop.line.naver.jp/products/0/0/1/{}/iphone/animation/{}@2x.png",
            pack_id, sticker.id
        );
        let apng_path = PathBuf::from(format!("{}/{}.apng", pack_name, sticker.id));
        let gif_path = PathBuf::from(format!("{}/{}.gif", pack_name, sticker.id));

        save_image(&apng_url, &apng_path).await;
        convert_apng_to_gif(&apng_path, &gif_path).expect("Failed to convert APNG to GIF");

        // Remove APNG file after conversion to GIF
        tokio::fs::remove_file(&apng_path)
            .await
            .expect("Failed to delete APNG file");
    }
}

/// Checks if an emoji pack provides an animation zip.
pub async fn emoji_has_animation(pack_id: &str) -> bool {
    let url = format!(
        "https://stickershop.line-scdn.net/sticonshop/v1/{}/sticon/iphone/package_animation.zip?v=1",
        pack_id
    );

    if let Ok(resp) = reqwest::get(&url).await {
        resp.status().is_success()
    } else {
        false
    }
}

async fn download_and_extract_emoji_zip(
    url: &str,
    pack_name: &str,
    subfolder: &str,
    wanted_ext: &str, // ".png" oder ".gif"
) {
    let resp = match reqwest::get(url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download {}: {}", url, e);
            return;
        }
    };

    if !resp.status().is_success() {
        eprintln!("Server returned {} for {}", resp.status(), url);
        return;
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read response body: {}", e);
            return;
        }
    };

    let cursor = Cursor::new(bytes);
    let mut zip = match ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("Failed to open zip: {}", e);
            return;
        }
    };

    let base = PathBuf::from(pack_name).join(subfolder);
    if let Err(e) = tokio::fs::create_dir_all(&base).await {
        eprintln!("Failed to create folder {:?}: {}", base, e);
        return;
    }

    for i in 0..zip.len() {
        let mut file = match zip.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to read file from zip: {}", e);
                continue;
            }
        };

        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        if !name.ends_with(wanted_ext) {
            continue;
        }

        let file_name = name.rsplit('/').next().unwrap_or(&name);
        let out_path = base.join(file_name);

        let mut out_file = match File::create(&out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to create {:?}: {}", out_path, e);
                continue;
            }
        };

        if let Err(e) = std::io::copy(&mut file, &mut out_file) {
            eprintln!("Failed to write {:?}: {}", out_path, e);
        }
    }

    println!("Extracted {} files from {}", wanted_ext, url);
}

/// Downloads static PNG emoji images from an emoji pack.
pub async fn download_emoji_static(pack_id: &str, pack_name: &str) {
    let url = format!(
        "https://stickershop.line-scdn.net/sticonshop/v1/{}/sticon/iphone/package.zip?v=1",
        pack_id
    );
    download_and_extract_emoji_zip(&url, pack_name, "png", ".png").await;
}

/// Downloads animated GIF emoji images from an emoji pack:
/// holt PNG/APNG aus package_animation.zip, wandelt sie in GIFs und legt sie im gif-Ordner ab.
pub async fn download_emoji_animated(pack_id: &str, pack_name: &str) {
    let url = format!(
        "https://stickershop.line-scdn.net/sticonshop/v1/{}/sticon/iphone/package_animation.zip",
        pack_id
    );

    let resp = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download emoji animation zip {}: {}", url, e);
            return;
        }
    };

    if !resp.status().is_success() {
        eprintln!(
            "Server returned {} for emoji animation zip {}",
            resp.status(),
            url
        );
        return;
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read emoji animation response body: {}", e);
            return;
        }
    };

    let cursor = Cursor::new(bytes);
    let mut zip = match ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("Failed to open emoji animation zip: {}", e);
            return;
        }
    };

    let gif_dir = PathBuf::from(pack_name).join("gif");
    if let Err(e) = tokio::fs::create_dir_all(&gif_dir).await {
        eprintln!("Failed to create gif folder {:?}: {}", gif_dir, e);
        return;
    }

    for i in 0..zip.len() {
        let mut file = match zip.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to read file from emoji animation zip: {}", e);
                continue;
            }
        };

        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        // animated emoji frames are APNG/PNG files inside the animation zip
        if !name.ends_with(".png") {
            continue;
        }

        let file_name = name.rsplit('/').next().unwrap_or(&name);
        let stem = Path::new(file_name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let apng_path = gif_dir.join(format!("{}.apng", stem));
        let gif_path = gif_dir.join(format!("{}.gif", stem));

        let mut apng_file = match File::create(&apng_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to create temp APNG file {:?}: {}", apng_path, e);
                continue;
            }
        };

        if let Err(e) = std::io::copy(&mut file, &mut apng_file) {
            eprintln!("Failed to write temp APNG file {:?}: {}", apng_path, e);
            continue;
        }

        if let Err(e) = convert_apng_to_gif(&apng_path, &gif_path) {
            eprintln!(
                "Failed to convert emoji APNG {:?} to GIF {:?}: {}",
                apng_path, gif_path, e
            );
        }

        if let Err(e) = tokio::fs::remove_file(&apng_path).await {
            eprintln!("Failed to delete temp APNG file {:?}: {}", apng_path, e);
        }
    }

    println!(
        "Animated emoji GIFs for pack {} downloaded to {:?}",
        pack_id, gif_dir
    );
}