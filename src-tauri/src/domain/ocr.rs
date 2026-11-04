use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OcrScreenBounds {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OcrCardAdvice {
    pub slot: u8,
    pub raw_text: String,
    pub confidence: f32,
    pub matched_name: Option<String>,
    pub candidate_name: Option<String>,
    pub grade: Option<String>,
    pub reason: Option<String>,
    #[serde(default)]
    pub win_rate: Option<f64>,
    #[serde(default)]
    pub games: Option<u64>,
    #[serde(default)]
    pub low_sample: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OcrOverlayPayload {
    pub champion_name: Option<String>,
    pub message: String,
    pub monitor: Option<OcrScreenBounds>,
    pub game_window: Option<OcrScreenBounds>,
    pub cards: Vec<OcrCardAdvice>,
}
