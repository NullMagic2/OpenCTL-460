//! Optional, language-independent handwriting heuristic using completed stroke summaries.
//! Scores are evidence, not calibrated probabilities or text recognition. No stroke history
//! is saved to disk; small drawings can look like writing. Mode changes latch at pen-down.

use std::collections::VecDeque;

#[derive(Clone, Copy)]
struct Stroke {
    start: f64,
    last: (f64, f64),
    direction: Option<(f64, f64)>,
    min: (f64, f64),
    max: (f64, f64),
    length: f64,
    turn: f64,
}

#[derive(Default)]
pub struct HandwritingDetector {
    stroke: Option<Stroke>,
    evidence: VecDeque<(f64, f64)>,
    last_lift: Option<f64>,
    last_time: Option<f64>,
    likely: bool,
}

impl HandwritingDetector {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Only completed strokes contribute: processing a current stroke never changes its mode.
    pub fn observe(&mut self, x: u16, y: u16, contact: bool, now_ms: f64) {
        if !now_ms.is_finite() || self.last_time.is_some_and(|t| now_ms < t) {
            return;
        }
        self.last_time = Some(now_ms);
        if self.last_lift.is_some_and(|t| now_ms - t > 1800.0) && self.stroke.is_none() {
            self.evidence.clear();
            self.likely = false;
        }
        if contact {
            let p = (f64::from(x) / 100.0, f64::from(y) / 100.0);
            if let Some(s) = &mut self.stroke {
                let d = (p.0 - s.last.0, p.1 - s.last.1);
                let distance = d.0.hypot(d.1);
                // Ignore sub-0.08 mm movements in shape evidence so sensor jitter is not curvature.
                if distance >= 0.08 {
                    let unit = (d.0 / distance, d.1 / distance);
                    if let Some(old) = s.direction {
                        s.turn += (old.0 * unit.0 + old.1 * unit.1).clamp(-1.0, 1.0).acos();
                    }
                    s.direction = Some(unit);
                    s.length += distance;
                    s.last = p;
                }
                s.min = (s.min.0.min(p.0), s.min.1.min(p.1));
                s.max = (s.max.0.max(p.0), s.max.1.max(p.1));
            } else {
                self.stroke = Some(Stroke {
                    start: now_ms,
                    last: p,
                    direction: None,
                    min: p,
                    max: p,
                    length: 0.0,
                    turn: 0.0,
                });
            }
        } else if let Some(s) = self.stroke.take() {
            let duration = now_ms - s.start;
            let extent = (s.max.0 - s.min.0).max(s.max.1 - s.min.1);
            let speed = s.length / (duration / 1000.0).max(0.001);
            let gap_ok = self.last_lift.is_some_and(|t| s.start - t <= 900.0);
            // A mixture of compact, briefly written, moving, curved strokes is suggestive only.
            let compact = (0.5..=18.0).contains(&extent);
            let plausible =
                compact && (50.0..=1600.0).contains(&duration) && (3.0..=300.0).contains(&speed);
            let score = if plausible {
                0.55 + if s.turn >= 0.7 { 0.30 } else { 0.0 } + if gap_ok { 0.15 } else { 0.0 }
            } else {
                0.0
            };
            self.evidence.push_back((now_ms, score));
            while self.evidence.len() > 8
                || self
                    .evidence
                    .front()
                    .is_some_and(|(t, _)| now_ms - t > 6000.0)
            {
                self.evidence.pop_front();
            }
            self.last_lift = Some(now_ms);
            let score = self.score();
            if self.evidence.len() >= 3 && score >= 0.78 {
                self.likely = true;
            } else if score < 0.5 {
                self.likely = false;
            }
        }
    }

    pub fn score(&self) -> f64 {
        if self.evidence.is_empty() {
            0.0
        } else {
            self.evidence.iter().map(|(_, s)| s).sum::<f64>() / self.evidence.len() as f64
        }
    }
    pub fn likely(&self) -> bool {
        self.likely
    }
}
