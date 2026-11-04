use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AugmentGradeRow {
    pub champion_id: String,
    pub augment_id: String,
    pub name: String,
    pub grade: Option<String>,
    pub reason: String,
    pub patch: Option<String>,
    pub win_rate: Option<f64>,
    pub games: Option<u64>,
    pub delta_pp: Option<f64>,
    pub low_sample: bool,
}
