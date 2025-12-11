mod metadata;
mod download;
mod conversion;
mod utils;

use std::io;

use metadata::{fetch_pack_metadata, fetch_emoji_pack_name};
use download::{
    check_for_animated_stickers,
    download_stickers,
    download_static_stickers,
    is_emoji_pack,
    emoji_has_animation,
    download_emoji_static,
    download_emoji_animated,
};
use utils::sanitize_and_create_folder;

#[tokio::main]
async fn main() {
    loop {
        println!("Enter the Sticker or Emoji Pack ID:");
        let mut pack_id_input = String::new();
        io::stdin()
            .read_line(&mut pack_id_input)
            .expect("Failed to read line");
        let pack_id = pack_id_input.trim().to_string();

        if is_emoji_pack(&pack_id) {
            // ---------- Emoji-Flow ----------
            println!("Detected emoji pack ID.");

            let emoji_title = fetch_emoji_pack_name(&pack_id).await;
            let pack_name = sanitize_and_create_folder(&emoji_title);

            let has_animation = emoji_has_animation(&pack_id).await;

            if has_animation {
                println!("This emoji pack contains animated emojis.");
                println!("Select the type of files you want to download: png, gif, or both");
                let mut file_type = String::new();
                io::stdin()
                    .read_line(&mut file_type)
                    .expect("Failed to read line");
                let file_type = file_type.trim().to_lowercase();

                match file_type.as_str() {
                    "png" => {
                        download_emoji_static(&pack_id, &pack_name).await;
                    }
                    "gif" => {
                        download_emoji_animated(&pack_id, &pack_name).await;
                    }
                    "both" => {
                        download_emoji_static(&pack_id, &pack_name).await;
                        download_emoji_animated(&pack_id, &pack_name).await;
                    }
                    _ => eprintln!("Invalid file type provided."),
                }
            } else {
                println!("This emoji pack contains only static PNG emojis. Downloading PNGs...");
                download_emoji_static(&pack_id, &pack_name).await;
            }
        } else {
            // ---------- Sticker-Flow ----------
            let pack_meta = fetch_pack_metadata(&pack_id).await;
            let pack_name = sanitize_and_create_folder(&pack_meta.title.en);

            let contains_animated =
                check_for_animated_stickers(&pack_id, &pack_meta.stickers).await;

            if contains_animated {
                println!("This sticker pack contains animated stickers.");
                println!("Select the type of files you want to download: png, gif, or both");
                let mut file_type = String::new();
                io::stdin()
                    .read_line(&mut file_type)
                    .expect("Failed to read line");
                let file_type = file_type.trim().to_lowercase();

                download_stickers(&pack_id, &pack_meta.stickers, &pack_name, &file_type).await;
            } else {
                println!("This pack contains only PNG stickers. Downloading PNGs...");
                download_static_stickers(&pack_meta.stickers, &pack_name).await;
            }
        }

        println!("Do you want to download another pack? (yes/no)");
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .expect("Failed to read line");
        if answer.trim().eq_ignore_ascii_case("no") {
            break;
        }
    }
}