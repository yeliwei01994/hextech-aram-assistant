use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AugmentGradeRow {
    pub champion_id: String,
    pub augment_id: String,
    pub name: String,
    pub grade: String,
    pub reason: String,
    pub patch: Option<String>,
}
