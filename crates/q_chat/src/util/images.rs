use std::fs;
use std::path::Path;

use crossterm::execute;
use crossterm::style::{
    self,
    Color,
};
use fig_api_client::model::{
    ImageBlock,
    ImageFormat,
    ImageSource,
};

use crate::consts::{
    MAX_IMAGE_SIZE,
    MAX_NUMBER_OF_IMAGES_PER_REQUEST,
};
use crate::shared_writer::SharedWriter;

#[derive(Clone, Debug)]
pub struct ImageMetadata {
    pub filepath: String,
    pub size: u64, // in bytes
    pub filename: String,
}

pub type RichImageBlocks = Vec<(ImageBlock, ImageMetadata)>;

pub type RichImageBlock = (ImageBlock, ImageMetadata);

/// This is the user facing function that handles the images from the user prompt.
/// It extracts the images from the user prompt and returns a vector of valid image data.
///
/// Image data is represented as a tuple of (ImageBlock, ImageMetadata).
///
/// It also handles printing necessary information about the images extracted and validation errors.
pub fn handle_images_from_user_prompt(output: &mut SharedWriter, user_prompt: &str) -> RichImageBlocks {
    let extracted_images = extract_images_from_user_prompt(user_prompt);

    let (mut valid_images, images_exceeding_size_limit): (RichImageBlocks, RichImageBlocks) = extracted_images
        .into_iter()
        .partition(|(_, metadata)| metadata.size as usize <= MAX_IMAGE_SIZE);

    if valid_images.len() > MAX_NUMBER_OF_IMAGES_PER_REQUEST {
        execute!(
            &mut *output,
            style::SetForegroundColor(Color::DarkYellow),
            style::Print(format!(
                "\nMore than {} images detected. Extra ones will be dropped.\n",
                MAX_NUMBER_OF_IMAGES_PER_REQUEST
            )),
            style::SetForegroundColor(Color::Reset)
        )
        .ok();
        valid_images.truncate(MAX_NUMBER_OF_IMAGES_PER_REQUEST);
    }

    if !images_exceeding_size_limit.is_empty() {
        execute!(
            &mut *output,
            style::SetForegroundColor(Color::DarkYellow),
            style::Print(format!(
                "\nThe following images are dropped due to exceeding size limit ({}MB):\n",
                MAX_IMAGE_SIZE / (1024 * 1024)
            )),
            style::SetForegroundColor(Color::Reset)
        )
        .ok();
        for (_, metadata) in &images_exceeding_size_limit {
            let image_size_str = if metadata.size > 1024 * 1024 {
                format!("{:.2} MB", metadata.size as f64 / (1024.0 * 1024.0))
            } else if metadata.size > 1024 {
                format!("{:.2} KB", metadata.size as f64 / 1024.0)
            } else {
                format!("{} bytes", metadata.size)
            };
            execute!(
                &mut *output,
                style::SetForegroundColor(Color::DarkYellow),
                style::Print(format!("  - {} ({})\n", metadata.filename, image_size_str)),
                style::SetForegroundColor(Color::Reset)
            )
            .ok();
        }
    }
    valid_images
}

/// Given a user prompt, this function extracts all the image paths from the prompt
/// and returns a vector of tuples containing the image block and the file path.
pub fn extract_images_from_user_prompt(user_prompt: &str) -> Vec<(ImageBlock, ImageMetadata)> {
    let args = shlex::split(user_prompt).unwrap_or_default();

    let mut image_blocks = Vec::new();
    let mut seen_args = std::collections::HashSet::new();

    for arg in args.iter() {
        // Skip if a path has already been seen
        // This is to avoid duplicates in the image blocks
        if seen_args.contains(arg) {
            continue;
        }
        seen_args.insert(arg);

        if is_supported_image_type(arg) {
            if let Some(image_block) = get_image_block_from_file_path(arg) {
                let path = arg;
                let filename = Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let image_size = fs::metadata(path).map(|m| m.len()).unwrap_or_default();

                image_blocks.push((image_block, ImageMetadata {
                    filename,
                    filepath: path.to_string(),
                    size: image_size,
                }));
            }
        }
    }

    image_blocks
}

/// This function checks if the file path has a supported image type
/// and returns true if it does, otherwise false.
/// Supported image types are: jpg, jpeg, png, gif, webp
///
/// # Arguments
///
/// * `maybe_file_path` - A string slice that may or may not be a valid file path
///
/// # Returns
///
/// * `true` if the file path has a supported image type
/// * `false` otherwise
pub fn is_supported_image_type(maybe_file_path: &str) -> bool {
    let supported_image_types = ["jpg", "jpeg", "png", "gif", "webp"];
    if let Some(extension) = maybe_file_path.split('.').last() {
        return supported_image_types.contains(&extension.trim().to_lowercase().as_str());
    }
    false
}

pub fn get_image_block_from_file_path(maybe_file_path: &str) -> Option<ImageBlock> {
    if !is_supported_image_type(maybe_file_path) {
        return None;
    }

    let file_path = Path::new(maybe_file_path);
    if !file_path.exists() {
        return None;
    }

    let image_bytes = fs::read(file_path);
    if image_bytes.is_err() {
        return None;
    }

    let image_format = get_image_format_from_ext(file_path.extension()?.to_str()?.to_lowercase().as_str());

    image_format.as_ref()?;

    let image_bytes = image_bytes.unwrap();
    let image_block = ImageBlock {
        format: image_format.unwrap(),
        source: ImageSource::Bytes(image_bytes),
    };
    Some(image_block)
}

pub fn get_image_format_from_ext(ext: &str) -> Option<ImageFormat> {
    match ext.trim().to_lowercase().as_str() {
        "jpg" => Some(ImageFormat::Jpeg),
        "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image_type() {
        assert!(is_supported_image_type("image.jpg"));
        assert!(is_supported_image_type("image.jpeg"));
        assert!(is_supported_image_type("image.png"));
        assert!(is_supported_image_type("image.gif"));
        assert!(is_supported_image_type("image.webp"));
        assert!(!is_supported_image_type("image.txt"));
        assert!(!is_supported_image_type("image"));
    }

    #[test]
    fn test_get_image_format_from_ext() {
        assert_eq!(get_image_format_from_ext("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(get_image_format_from_ext("JPEG"), Some(ImageFormat::Jpeg));
        assert_eq!(get_image_format_from_ext("png"), Some(ImageFormat::Png));
        assert_eq!(get_image_format_from_ext("gif"), Some(ImageFormat::Gif));
        assert_eq!(get_image_format_from_ext("webp"), Some(ImageFormat::Webp));
        assert_eq!(get_image_format_from_ext("txt"), None);
    }

    #[test]
    fn test_extract_images_from_user_prompt() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image.jpg");
        std::fs::write(&image_path, b"fake_image_data").unwrap();

        let user_prompt = format!("{} and some unrelated text", image_path.to_string_lossy());
        let images = extract_images_from_user_prompt(&user_prompt);

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].1.filename, "test_image.jpg");
        assert_eq!(images[0].1.filepath, image_path.to_string_lossy());
    }

    #[test]
    fn test_handle_images_from_user_prompt() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image.jpg");
        std::fs::write(&image_path, b"fake_image_data").unwrap();

        let mut output = SharedWriter::stdout();
        let user_prompt = format!("{}", image_path.to_string_lossy());
        let images = handle_images_from_user_prompt(&mut output, &user_prompt);

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].1.filename, "test_image.jpg");
        assert_eq!(images[0].1.filepath, image_path.to_string_lossy());
    }

    #[test]
    fn test_get_image_block_from_file_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test_image.png");
        std::fs::write(&image_path, b"fake_image_data").unwrap();

        let image_block = get_image_block_from_file_path(&image_path.to_string_lossy());
        assert!(image_block.is_some());
        let image_block = image_block.unwrap();
        assert_eq!(image_block.format, ImageFormat::Png);
        if let ImageSource::Bytes(bytes) = image_block.source {
            assert_eq!(bytes, b"fake_image_data");
        } else {
            panic!("Expected ImageSource::Bytes");
        }
    }

    #[test]
    fn test_handle_images_size_limit_exceeded() {
        let temp_dir = tempfile::tempdir().unwrap();
        let large_image_path = temp_dir.path().join("large_image.jpg");
        let large_image_size = MAX_IMAGE_SIZE as usize + 1;
        std::fs::write(&large_image_path, vec![0; large_image_size]).unwrap();

        let mut output = SharedWriter::stdout();
        let user_prompt = format!("{}", large_image_path.to_string_lossy());
        let images = handle_images_from_user_prompt(&mut output, &user_prompt);

        assert!(images.is_empty());
    }

    #[test]
    fn test_handle_images_number_exceeded() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut user_prompt = String::new();

        for i in 0..(MAX_NUMBER_OF_IMAGES_PER_REQUEST + 2) {
            let image_path = temp_dir.path().join(format!("image_{}.jpg", i));
            std::fs::write(&image_path, b"fake_image_data").unwrap();
            user_prompt.push_str(&format!("{} ", image_path.to_string_lossy()));
        }

        let mut output = SharedWriter::stdout();
        let images = handle_images_from_user_prompt(&mut output, &user_prompt);

        assert_eq!(images.len(), MAX_NUMBER_OF_IMAGES_PER_REQUEST);
    }
}
