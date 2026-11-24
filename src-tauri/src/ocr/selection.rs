use crate::domain::{OcrCardAdvice, OcrScreenBounds};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct SelectionTracker {
    screen: Option<ChoiceScreen>,
    pending: Option<(u8, Instant)>,
    recent_click: Option<Click>,
    closing: bool,
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
        let same_screen = self.screen.as_ref().is_some_and(|screen| {
            screen.foreground_window == foreground_window
                && screen.bounds.left == bounds.left
                && screen.bounds.top == bounds.top
                && screen.bounds.width == bounds.width
                && screen.bounds.height == bounds.height
                && screen.cards.len() == cards.len()
                && screen.cards.iter().zip(&cards).all(|(old, new)| {
                    old.slot == new.slot
                        && old
                            .matched_name
                            .as_ref()
                            .zip(new.matched_name.as_ref())
                            .is_none_or(|(old, new)| old == new)
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
                    clicked_slot(&screen, click.x, click.y).map(|slot| (slot, click.at))
                });
        }
        self.screen = Some(screen);
    }

    pub(crate) fn disarm(&mut self) {
        self.screen = None;
        self.pending = None;
        self.recent_click = None;
        self.closing = false;
        self.escape_at = None;
    }

    pub(crate) fn observe_escape(&mut self) {
        self.escape_at = Some(Instant::now());
        self.cancel_click();
    }

    pub(crate) fn cancel_click(&mut self) {
        self.pending = None;
        self.recent_click = None;
        self.closing = false;
    }

    pub(crate) fn set_closing(&mut self, closing: bool) {
        self.closing = closing;
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
        let clicked_card = self.pending.and_then(|(slot, at)| {
            if at.elapsed() > Duration::from_secs(5) {
                return None;
            }
            self.screen
                .as_ref()?
                .cards
                .iter()
                .find(|card| card.slot == slot)?
                .matched_name
                .as_deref()
        });
        if let Some(name) = hud_card.as_deref().or(clicked_card) {
            if !self.selected.iter().any(|selected| selected == name) {
                let source = match (hud_card.as_deref(), clicked_card) {
                    (Some(hud), Some(clicked)) if hud == clicked => "点击坐标 + HUD",
                    (Some(_), Some(_)) => "HUD 图标，点击候选与 HUD 不一致",
                    (Some(_), None) => "HUD 图标",
                    (None, _) => "点击坐标",
                };
                eprintln!("海克斯联动：已记录选卡 {name}（{source}）");
                self.selected.push(name.to_owned());
            }
        } else {
            let reason = match (self.screen.is_some(), self.pending) {
                (false, _) => "当前没有可用的卡位映射",
                (_, None) => "没有可用的卡内点击",
                (_, Some((_, at))) if at.elapsed() > Duration::from_secs(5) => "点击已超时",
                (_, Some(_)) => "点中的卡位没有匹配到卡名",
            };
            eprintln!("海克斯联动：选卡界面已关闭，未记录（{reason}）");
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
        self.record_click(Click {
            window: input::foreground_window(),
            x,
            y,
            at: Instant::now(),
        });
    }

    fn record_click(&mut self, click: Click) {
        if self.closing
            || self
                .escape_at
                .is_some_and(|at| click.at.saturating_duration_since(at) < Duration::from_secs(1))
        {
            return;
        }
        self.recent_click = Some(click);
        let Some(screen) = &self.screen else { return };
        if click.window != screen.foreground_window {
            return;
        }
        self.pending = clicked_slot(screen, click.x, click.y).map(|slot| (slot, click.at));
        if let Some((slot, _)) = self.pending {
            eprintln!("海克斯联动：点击命中卡位 {slot}，等待选卡界面关闭");
        } else {
            eprintln!("海克斯联动：点击未命中卡片区域");
        }
    }
}

fn clicked_slot(screen: &ChoiceScreen, x: i32, y: i32) -> Option<u8> {
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
        let center = card.center_x.unwrap_or_else(|| {
            if screen.cards.len() == 4 {
                0.185 + f64::from(card.slot.saturating_sub(1)) * 0.210
            } else {
                0.308 + f64::from(card.slot.saturating_sub(1)) * 0.1925
            }
        });
        ((x - center).abs() < half_width).then_some(card.slot)
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
        assert_eq!(clicked_slot(&screen, 811, 460), Some(1));
        assert_eq!(clicked_slot(&screen, 811, 780), None);
        screen.cards = vec![
            card(1, Some(0.185), "冰寒"),
            card(2, Some(0.395), "易损"),
            card(3, Some(0.605), "秘术冲拳"),
            card(4, Some(0.815), "虚幻武器"),
        ];
        assert_eq!(clicked_slot(&screen, 1785, 460), Some(4));
        assert_eq!(clicked_slot(&screen, 1620, 460), Some(4));
        assert_eq!(clicked_slot(&screen, 1584, 460), None);
    }

    #[test]
    fn escape_reroll_and_expired_clicks_do_not_confirm() {
        let bounds = OcrScreenBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let mut tracker = SelectionTracker::default();
        for reason in ["escape", "reroll", "stale", "outside"] {
            let now = Instant::now();
            let cards = vec![
                card(1, None, "冰寒"),
                card(2, None, "易损"),
                card(3, None, "虚幻武器"),
            ];
            tracker.arm(bounds.clone(), cards, now, 1);
            let at = if reason == "stale" {
                now - Duration::from_secs(6)
            } else {
                now
            };
            tracker.record_click(Click {
                window: 1,
                x: 962,
                y: 460,
                at,
            });
            match reason {
                "escape" => tracker.observe_escape(),
                "reroll" => tracker.disarm(),
                "outside" => tracker.record_click(Click {
                    window: 1,
                    x: 962,
                    y: 825,
                    at,
                }),
                _ => {}
            }
            tracker.confirm_closed();
            assert!(tracker.selected().is_empty(), "{reason}");
        }
    }

    #[test]
    fn click_on_unread_card_keeps_its_slot_until_ocr_refresh() {
        let bounds = OcrScreenBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let mut cards = vec![
            card(1, None, "冰寒"),
            card(2, None, "易损"),
            card(3, None, "虚幻武器"),
        ];
        cards[1].matched_name = None;
        let mut tracker = SelectionTracker::default();
        let started = Instant::now();
        tracker.record_click(Click {
            window: 1,
            x: 962,
            y: 460,
            at: Instant::now(),
        });
        tracker.arm(bounds.clone(), cards.clone(), started, 1);
        cards[1].matched_name = Some("易损".into());
        tracker.arm(bounds, cards, Instant::now(), 1);
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["易损"]);
    }

    #[test]
    fn hud_can_confirm_without_click_but_unread_slot_cannot() {
        let bounds = OcrScreenBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let mut cards = vec![
            card(1, None, "冰寒"),
            card(2, None, "易损"),
            card(3, None, "虚幻武器"),
        ];
        cards[1].matched_name = None;
        let mut tracker = SelectionTracker::default();
        tracker.arm(bounds.clone(), cards.clone(), Instant::now(), 1);
        tracker.record_click(Click {
            window: 1,
            x: 962,
            y: 460,
            at: Instant::now(),
        });
        tracker.confirm_closed();
        assert!(tracker.selected().is_empty());

        tracker.arm(bounds, cards, Instant::now(), 1);
        tracker.confirm_closed_with_hud(Some("虚幻武器"));
        assert_eq!(tracker.selected(), ["虚幻武器"]);
    }

    #[test]
    fn clicks_after_the_choice_disappears_do_not_replace_the_pick() {
        let bounds = OcrScreenBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let cards = vec![
            card(1, None, "冰寒"),
            card(2, None, "易损"),
            card(3, None, "虚幻武器"),
        ];
        let mut tracker = SelectionTracker::default();
        let now = Instant::now();
        tracker.arm(bounds, cards, now, 1);
        tracker.record_click(Click {
            window: 1,
            x: 962,
            y: 460,
            at: now,
        });
        tracker.set_closing(true);
        tracker.record_click(Click {
            window: 1,
            x: 592,
            y: 460,
            at: now,
        });
        tracker.confirm_closed();
        assert_eq!(tracker.selected(), ["易损"]);
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
