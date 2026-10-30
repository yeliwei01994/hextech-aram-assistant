use crate::domain::OcrScreenBounds;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OcrWorkerRequest {
    pub action: String,
    pub language: String,
}

#[derive(Deserialize)]
pub struct OcrWorkerResponse {
    #[serde(default)]
    pub screen_width: u32,
    #[serde(default)]
    pub screen_height: u32,
    pub monitor: Option<OcrScreenBounds>,
    pub game_window: Option<OcrScreenBounds>,
    #[serde(default)]
    pub cards: Vec<OcrRawCard>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct OcrProbeResponse {
    #[serde(default)]
    pub game_foreground: bool,
    #[serde(default)]
    pub augment_choice: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OcrRawCard {
    pub slot: u8,
    #[serde(default)]
    pub raw_text: String,
    #[serde(default)]
    pub confidence: f32,
}

pub struct OcrScreenResult {
    pub screen_width: u32,
    pub screen_height: u32,
    pub monitor: Option<OcrScreenBounds>,
    pub game_window: Option<OcrScreenBounds>,
    pub cards: Vec<OcrRawCard>,
}

pub struct OcrProbeResult {
    pub game_foreground: bool,
    pub augment_choice: bool,
}
