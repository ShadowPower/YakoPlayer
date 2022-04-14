#[derive(Debug, Default)]
pub struct MediaInfo {
    pub duration: i64,
    pub bitrate: i64,
    pub cover: Option<Vec<u8>>,
}