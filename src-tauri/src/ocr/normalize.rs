#[derive(Clone, Debug)]
pub struct NameMatch<'a> {
    pub name: &'a str,
    pub similarity: f32,
}

const OCR_MATCH_THRESHOLD: f32 = 0.82;
const CANDIDATE_THRESHOLD: f32 = 0.38;

pub fn match_augment_name<'a>(raw_text: &str, known_names: &'a [String]) -> Option<NameMatch<'a>> {
    let source = normalize(raw_text);
    if source.is_empty() {
        return None;
    }
    known_names
        .iter()
        .map(|name| NameMatch {
            name,
            similarity: similarity(&source, &normalize(name)),
        })
        .filter(|matched| matched.similarity >= CANDIDATE_THRESHOLD)
        .max_by(|a, b| a.similarity.total_cmp(&b.similarity))
}

pub fn is_safe_name_match(similarity: f32) -> bool {
    similarity >= OCR_MATCH_THRESHOLD
}

fn normalize(value: &str) -> Vec<char> {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace()
                && !character.is_ascii_punctuation()
                && !matches!(
                    character,
                    '，' | '。'
                        | '？'
                        | '！'
                        | '：'
                        | '；'
                        | '、'
                        | '·'
                        | '…'
                        | '—'
                        | '（'
                        | '）'
                        | '【'
                        | '】'
                        | '《'
                        | '》'
                )
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn similarity(left: &[char], right: &[char]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (left_index, left_char) in left.iter().enumerate() {
        let mut current = vec![left_index + 1; right.len() + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
        }
        previous = current;
    }
    let edit_distance = previous[right.len()];
    1.0 - edit_distance as f32 / left.len().max(right.len()) as f32
}

pub fn is_safe_name_match_for_overlay(similarity: f32, confidence: f32) -> bool {
    confidence >= 0.80 && is_safe_name_match(similarity)
}
