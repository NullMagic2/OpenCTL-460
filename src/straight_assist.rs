//! Small, bounded cross-stroke correction after a nearly straight direction emerges.
//! Spatial sampling makes the result independent of timer repeats and drawing speed.
//! One-sided curvature is rejected; this never snaps to a screen axis or an endpoint.

type Point = (f64, f64);
const SPACING: f64 = 16.0; // Sensor units: 100 units per physical tablet millimetre.
const LIMIT: f64 = 12.0;
#[derive(Default)]
pub struct StraightAssist {
    points: [Point; 17],
    count: usize,
    previous: Option<Point>,
    distance: f64,
    offset: Point,
}
impl StraightAssist {
    pub fn reset(&mut self) -> Point {
        let offset = self.offset;
        *self = Self::default();
        offset
    }
    fn push(&mut self, p: Point) {
        if self.count == self.points.len() {
            self.points.copy_within(1.., 0);
            self.points[self.count - 1] = p;
        } else {
            self.points[self.count] = p;
            self.count += 1;
        }
    }
    pub fn update(&mut self, p: Point, strength: f64) -> Point {
        let Some(previous) = self.previous.replace(p) else {
            self.push(p);
            return p;
        };
        let delta = (p.0 - previous.0, p.1 - previous.1);
        let distance = delta.0.hypot(delta.1);
        if distance > 512.0 {
            self.reset();
            self.previous = Some(p);
            self.push(p);
            return p;
        }
        if distance > 0.0 {
            let mut along = SPACING - self.distance;
            while along <= distance {
                self.push((
                    previous.0 + delta.0 * along / distance,
                    previous.1 + delta.1 * along / distance,
                ));
                along += SPACING;
            }
            self.distance = (self.distance + distance) % SPACING;
            let desired = self.correction(p, strength.clamp(0.0, 0.9));
            // Ease acquisition/release by travelled distance, never by stationary timer ticks.
            let change = (desired.0 - self.offset.0, desired.1 - self.offset.1);
            let length = change.0.hypot(change.1);
            let fraction = if length > 0.0 {
                (distance * 0.15 / length).min(1.0)
            } else {
                0.0
            };
            self.offset.0 += change.0 * fraction;
            self.offset.1 += change.1 * fraction;
        }
        (p.0 + self.offset.0, p.1 + self.offset.1)
    }
    fn correction(&self, p: Point, strength: f64) -> Point {
        if self.count < self.points.len() || strength == 0.0 {
            return (0.0, 0.0);
        }
        let first = self.points[0];
        let last = self.points[self.count - 1];
        let chord = (last.0 - first.0, last.1 - first.1);
        let span = chord.0.hypot(chord.1);
        if span < 220.0 {
            return (0.0, 0.0);
        } // Reject turns, reversals, and compact loops.
        let normal = (-chord.1 / span, chord.0 / span);
        let mut minimum: f64 = 0.0;
        let mut maximum: f64 = 0.0;
        for q in self.points {
            let cross = (q.0 - first.0) * normal.0 + (q.1 - first.1) * normal.1;
            minimum = minimum.min(cross);
            maximum = maximum.max(cross);
        }
        // A clean arc stays on one side of its chord; leave it alone. A straight
        // noisy stroke crosses the chord. Sub-unit quantization needs no correction.
        if minimum >= -1.0 || maximum <= 1.0 || maximum - minimum > 24.0 {
            return (0.0, 0.0);
        }
        let mean = self
            .points
            .iter()
            .fold((0.0, 0.0), |s, q| (s.0 + q.0 / 17.0, s.1 + q.1 / 17.0));
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for q in self.points {
            let (x, y) = (q.0 - mean.0, q.1 - mean.1);
            xx += x * x;
            xy += x * y;
            yy += y * y;
        }
        let angle = 0.5 * (2.0 * xy).atan2(xx - yy);
        let n = (-angle.sin(), angle.cos());
        let cross = (p.0 - mean.0) * n.0 + (p.1 - mean.1) * n.1;
        let correction = (cross * strength).clamp(-LIMIT, LIMIT);
        (-n.0 * correction, -n.1 * correction)
    }
}
