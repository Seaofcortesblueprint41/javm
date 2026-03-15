use nom_exif::{EntryValue, MediaParser, MediaSource, TrackInfo, TrackInfoTag};
use std::path::Path;

pub struct VideoMetadata {
    pub duration: Option<u64>, // Duration in seconds
    pub width: Option<u64>,
    pub height: Option<u64>,
}

fn val_to_u64(v: &EntryValue) -> Option<u64> {
    match v {
        EntryValue::U8(x) => Some(*x as u64),
        EntryValue::U16(x) => Some(*x as u64),
        EntryValue::U32(x) => Some(*x as u64),
        EntryValue::U64(x) => Some(*x),
        // If we encounter other types (Rational), we might need to handle them.
        // For video duration/dimensions, integer types are standard in nom-exif.
        _ => None,
    }
}

pub fn extract_metadata(path: &Path) -> Result<VideoMetadata, String> {
    let mut parser = MediaParser::new();
    let ms = MediaSource::file_path(path).map_err(|e| e.to_string())?;

    let info: TrackInfo = parser.parse(ms).map_err(|e| e.to_string())?;

    // DurationMs is the key, value in milliseconds
    let duration = info
        .get(TrackInfoTag::DurationMs)
        .and_then(val_to_u64)
        .map(|d| d / 1000);

    let width = info.get(TrackInfoTag::ImageWidth).and_then(val_to_u64);

    let height = info.get(TrackInfoTag::ImageHeight).and_then(val_to_u64);

    Ok(VideoMetadata {
        duration,
        width,
        height,
    })
}
