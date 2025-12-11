use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StickerPack {
    pub title: Title,
    pub stickers: Vec<Sticker>,
}

#[derive(Debug, Deserialize)]
pub struct Title {
    pub en: String,
}

#[derive(Debug, Deserialize)]
pub struct Sticker {
    pub id: u32,
}

/// Fetches the metadata of a sticker pack from the given pack ID.
pub async fn fetch_pack_metadata(pack_id: &str) -> StickerPack {
    let url = format!(
        "https://dl.stickershop.line.naver.jp/products/0/0/1/{}/android/productInfo.meta",
        pack_id
    );

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build client");

    let response = client
        .get(&url)
        .send()
        .await
        .expect("Failed to fetch metadata");

    if response.status().is_success() {
        response.json().await.expect("Failed to parse JSON")
    } else {
        eprintln!("Error fetching metadata for pack ID: {}", pack_id);
        std::process::exit(1);
    }
}

/// Fetches the display name of an emoji pack from the LINE Store page.
/// Falls back to the pack_id if the name cannot be determined.
pub async fn fetch_emoji_pack_name(pack_id: &str) -> String {
    let url = format!(
        "https://store.line.me/emojishop/product/{}/en",
        pack_id
    );

    let resp = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to fetch emoji pack page {}: {}", url, e);
            return pack_id.to_string();
        }
    };

    let body = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to read emoji pack page body: {}", e);
            return pack_id.to_string();
        }
    };

    // Try to extract the pack name from the og:title meta tag:
    // <meta property="og:title" content="Something small and cute – LINE Emoji | LINE STORE">
    if let Some(idx) = body.find("og:title\" content=\"") {
        let start = idx + "og:title\" content=\"".len();
        if let Some(rest) = body.get(start..) {
            if let Some(end) = rest.find('"') {
                let full_title = &rest[..end];
                return if let Some(split) = full_title.find(" – ") {
                    full_title[..split].to_string()
                } else {
                    full_title.to_string()
                }
            }
        }
    }

    // Fallback: just return the pack_id string
    pack_id.to_string()
}