use super::protocol::OcrCardProbeMetric;
use crate::domain::{OcrCardAdvice, OcrScreenBounds};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct SelectionTracker {
    screen: Option<ChoiceScreen>,
    pending: Option<(String, Instant)>,
    recent_click: Option<Click>,
    highlighted: Option<(String, Instant)>,
    hovered: Option<(String, u8)>,
    closing_hover: Option<(String, Instant)>,
    escape_at: Option<Instant>,
    selected: Vec<String>,
}

#[derive(Clone, Copy)]
struct Click {
    window: usize,
    x: i32,
    y: i32,
    at: Instant,
}

struct ChoiceScreen {
    bounds: OcrScreenBounds,
    cards: Vec<OcrCardAdvice>,
    foreground_window: usize,
}

impl SelectionTracker {
    pub(crate) fn arm(
        &mut self,
        bounds: OcrScreenBounds,
        cards: Vec<OcrCardAdvice>,
        capture_started: Instant,
        foreground_window: usize,
    ) {
        if foreground_window == 0 || !(3..=4).contains(&cards.len()) {
            eprintln!(
                "海克斯联动：无法开始跟踪，窗口有效={}，卡片数={}",
                foreground_window != 0,
                cards.len()
            );
            return;
        }
        // An OCR refresh of the same choices must preserve an in-flight click.
        let same_screen =
            self.screen.as_ref().is_some_and(|screen| {
                screen.foreground_window == foreground_window
                    && screen.bounds.left == bounds.left
                    && screen.bounds.top == bounds.top
                    && screen.bounds.width == bounds.width
                    && screen.bounds.height == bounds.height
                    && screen.cards.len() == cards.len()
                    && screen.cards.iter().zip(&cards).all(|(old, new)| {
                        old.slot == new.slot && old.matched_name == new.matched_name
                    })
            });
        if !same_screen {
            eprintln!(
                "海克斯联动：开始跟踪 {} 张卡，已识别 {} 张",
                cards.len(),
                cards
                    .iter()
                    .filter(|card| card.matched_name.is_some())
                    .count()
            );
            self.pending = None;
            self.highlighted = None;
            self.hovered = None;
            self.closing_hover = None;
        }
        let screen = ChoiceScreen {
            bounds,
            cards,
            foreground_window,
        };
        if self.pending.is_none() {
            self.pending = self
                .recent_click
                .filter(|click| click.window == foreground_window && click.at >= capture_started)
                .and_then(|click| {
                    clicked_card(&screen, click.x, click.y).map(|name| (name.to_owned(), click.at))
                });
        }
        self.screen = Some(screen);
    }

    pub(crate) fn disarm(&mut self) {
        self.screen = None;
        self.pending = None;
        self.recent_click = None;
        self.highlighted = None;
        self.hovered = None;
        self.closing_hover = None;
        self.escape_at = None;
    }

    pub(crate) fn observe_escape(&mut self) {
        self.escape_at = Some(Instant::now());
        self.cancel_inference();
    }

    pub(crate) fn observe_pointer(&mut self) {
        self.closing_hover = None;
        let previous = self.hovered.take();
        self.hovered = self.card_under_pointer().map(|name| {
            let count = previous
                .as_ref()
                .filter(|(old, _)| old == name)
                .map_or(1, |(_, count)| count.saturating_add(1));
            (name.to_owned(), count)
        });
    }

    pub(crate) fn observe_closing_frame(&mut self) {
        self.closing_hover = self
            .hovered
            .as_ref()
            .filter(|(name, count)| *count >= 2 && self.card_under_pointer() == Some(name.as_str()))
            .map(|(name, _)| (name.clone(), Instant::now()));
    }

    pub(crate) fn cancel_inference(&mut self) {
        self.hovered = None;
        self.closing_hover = None;
        self.highlighted = None;
    }

    fn card_under_pointer(&self) -> Option<&str> {
        let screen = self.screen.as_ref()?;
        if input::foreground_window() != screen.foreground_window {
            return None;
        }
        let (x, y) = input::cursor_position()?;
        clicked_card(screen, x, y)
    }

    pub(crate) fn observe_highlight(&mut self, metrics: &[OcrCardProbeMetric]) {
        self.highlighted = self.screen.as_ref().and_then(|screen| {
            let slot = highlighted_slot(metrics, screen.cards.len())?;
            let name = screen
                .cards
                .iter()
                .find(|card| card.slot == slot)?
                .matched_name
                .clone()?;
            Some((name, Instant::now()))
        });
    }

    #[cfg(test)]
    fn confirm_closed(&mut self) {
        self.confirm_closed_with_hud(None);
    }

    pub(crate) fn confirm_closed_with_hud(&mut self, hud_name: Option<&str>) {
        let hud_card = hud_name.and_then(|name| {
            let names: Vec<_> = self
                .screen
                .as_ref()?
                .cards
                .iter()
                .filter_map(|card| card.matched_name.clone())
                .collect();
            let found = super::normalize::match_augment_name(name, &names)?;
            (found.similarity == 1.0 && !self.selected.iter().any(|name| name == found.name))
                .then(|| found.name.to_owned())
        });
        if let Some(name) = hud_card {
            eprintln!("海克斯联动：已记录选卡 {name}（HUD 图标确认）");
            self.selected.push(name);
            self.disarm();
            return;
        }
        let click = self
            .pending
            .take()
            .filter(|(_, at)| at.elapsed() <= Duration::from_secs(5));
        let inferred = click.is_none();
        let picked = click.or_else(|| {
            if self
                .escape_at
                .is_some_and(|at| at.elapsed() <= Duration::from_secs(3))
            {
                return None;
            }
            self.highlighted
                .take()
                .filter(|(_, at)| at.elapsed() <= Duration::from_secs(3))
                .or_else(|| {
                    self.closing_hover
                        .take()
                        .filter(|(_, at)| at.elapsed() <= Duration::from_secs(3))
                })
        });
        if let Some((name, _)) = picked {
            if !self.selected.contains(&name) {
                let source = if inferred { "（画面推断）" } else { "" };
                eprintln!("海克斯联动：已记录选卡 {name}{source}");
                self.selected.push(name);
            }
        } else {
            eprintln!(
                "海克斯联动：选卡界面已关闭，未确认选中卡片（跟踪={}）",
                self.screen.is_some()
            );
        }
        self.disarm();
    }

    pub(crate) fn selected(&self) -> &[String] {
        &self.selected
    }

    pub(crate) fn sample_click(&mut self) {
        let Some((x, y)) = input::cursor_position() else {
            return;
        };
        let click = Click {
            window: input::foreground_window(),
            x,
            y,
            at: Instant::now(),
        };
        self.recent_click = Some(click);
        let Some(screen) = &self.screen else { return };
        if click.window != screen.foreground_window {
            return;
        }
        if let Some(name) = clicked_card(screen, click.x, click.y) {
            self.pending = Some((name.to_string(), click.at));
        }
    }
}

fn highlighted_slot(metrics: &[OcrCardProbeMetric], count: usize) -> Option<u8> {
    if metrics.len() != count || !metrics.iter().all(|card| card.matched) {
        return None;
    }
    let brightest = metrics
        .iter()
        .max_by(|a, b| a.bright_border_ratio.total_cmp(&b.bright_border_ratio))?;
    (brightest.bright_border_ratio >= 0.65
        && metrics
            .iter()
            .filter(|card| card.slot != brightest.slot)
            .all(|card| brightest.bright_border_ratio - card.bright_border_ratio >= 0.25))
    .then_some(brightest.slot)
}

fn clicked_card(screen: &ChoiceScreen, x: i32, y: i32) -> Option<&str> {
    // Match the OCR worker's centered 16:9 reference, including ultrawide windows.
    let bounds = &screen.bounds;
    let width = f64::from(bounds.width).min(f64::from(bounds.height) * 16.0 / 9.0);
    let height = f64::from(bounds.height).min(width * 9.0 / 16.0);
    let x =
        (f64::from(x) - f64::from(bounds.left) - (f64::from(bounds.width) - width) / 2.0) / width;
    let y =
        (f64::from(y) - f64::from(bounds.top) - (f64::from(bounds.height) - height) / 2.0) / height;
    if !(0.177..0.668).contains(&y) {
        return None;
    }
    let half_width = if screen.cards.len() == 4 { 0.09 } else { 0.084 };
    screen.cards.iter().find_map(|card| {
        let center = card.center_x.unwrap_or_else(|| match card.slot {
            1 => 0.308,
            2 => 0.501,
            3 => 0.693,
            _ => 0.815,
        });
        ((x - center).abs() < half_width)
            .then_some(card.matched_name.as_deref())
            .flatten()
    })
}

#[cfg(windows)]
pub(crate) mod input {
    use std::ffi::c_void;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
        fn GetPhysicalCursorPos(point: *mut Point) -> i32;
        fn GetForegroundWindow() -> *mut c_void;
    }

    pub(crate) fn foreground_window() -> usize {
        unsafe { GetForegroundWindow() as usize }
    }

    pub(crate) fn left_down() -> bool {
        unsafe { GetAsyncKeyState(1) < 0 }
    }

    pub(crate) fn escape_down() -> bool {
        unsafe { GetAsyncKeyState(0x1b) < 0 }
    }

    pub(super) fn cursor_position() -> Option<(i32, i32)> {
        let mut point = Point { x: 0, y: 0 };
        (unsafe { GetPhysicalCursorPos(&mut point) } != 0).then_some((point.x, point.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(slot: u8, center_x: Option<f64>, name: &str) -> OcrCardAdvice {
        OcrCardAdvice {
            center_x,
            slot,
            raw_text: String::new(),
            confidence: 1.0,
            matched_name: Some(name.into()),
            candidate_name: None,
            grade: None,
            reason: None,
            win_rate: None,
            games: None,
            low_sample: false,
        }
    }

    #[test]
    fn maps_three_and_four_cards_without_counting_reroll_clicks() {
        let mut screen = ChoiceScreen {
            bounds: OcrScreenBounds {
                left: -100,
                top: 20,
                width: 2560,
                height: 1080,
            },
            cards: vec![
                card(1, None, "虚幻武器"),
                card(2, None, "冰寒"),
                card(3, None, "易损"),
            ],
            foreground_window: 1,
        };
        assert_eq!(clicked_card(&screen, 811, 460), Some("虚幻武器"));
        assert_eq!(clicked_card(&screen, 811, 780), None);
        screen.cards = vec![
            card(1, Some(0.185), "冰寒"),
            card(2, Some(0.395), "易损"),
            card(3, Some(0.605), "秘术冲拳"),
            card(4, Some(0.815), "虚幻武器"),
        ];
        assert_eq!(clicked_card(&screen, 1785, 460), Some("虚幻武器"));
        assert_eq!(clicked_card(&screen, 1620, 460), Some("虚幻武器"));
        assert_eq!(clicked_card(&screen, 1584, 460), None);
    }

    #[test]
    fn records_only_recent_clicks_confirmed_by_closure() {
        let mut tracker = SelectionTracker::default();
        tracker.pending = Some(("虚幻武器".into(), Instant::now()));
        assert!(tracker.selected().is_empty());
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["虚幻武器"]);
        tracker.pending = Some(("易损".into(), Instant::now() - Duration::from_secs(6)));
        tracker.confirm_closed();
        tracker.pending = Some(("冰寒".into(), Instant::now()));
        tracker.disarm();
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["虚幻武器"]);
    }

    #[test]
    fn closing_hover_is_a_fallback_and_escape_blocks_inference() {
        let mut tracker = SelectionTracker::default();
        tracker.closing_hover = Some(("珠光护手".into(), Instant::now()));
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["珠光护手"]);

        tracker.observe_escape();
        tracker.closing_hover = Some(("冰寒".into(), Instant::now()));
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["珠光护手"]);

        tracker.closing_hover = Some(("冰寒".into(), Instant::now()));
        tracker.pending = Some(("易损".into(), Instant::now()));
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["珠光护手", "易损"]);
    }

    #[test]
    fn accepts_only_a_unique_highlight() {
        let metrics = |ratios: &[f32]| {
            ratios
                .iter()
                .enumerate()
                .map(|(i, ratio)| OcrCardProbeMetric {
                    slot: i as u8 + 1,
                    bright_border_ratio: *ratio,
                    matched: true,
                    ..Default::default()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(highlighted_slot(&metrics(&[0.70, 0.28, 0.31]), 3), Some(1));
        assert_eq!(highlighted_slot(&metrics(&[0.70, 0.65, 0.31]), 3), None);
        assert_eq!(highlighted_slot(&metrics(&[0.28, 0.28, 0.31]), 3), None);
        assert_eq!(highlighted_slot(&metrics(&[0.70, 0.28, 0.31]), 4), None);
    }
}

#[cfg(not(windows))]
pub(crate) mod input {
    pub(crate) fn foreground_window() -> usize {
        0
    }
    pub(crate) fn left_down() -> bool {
        false
    }
    pub(super) fn cursor_position() -> Option<(i32, i32)> {
        None
    }
}
