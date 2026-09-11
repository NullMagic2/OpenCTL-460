//! Optional artistic tilt synthesis from stroke heading, speed and pressure.
//! This estimates a useful shading gesture, NOT physical pen orientation.
//! Heading is an undirected axis: stroke reversals do not flip shading by 180 degrees.

#[derive(Default)]
pub struct VirtualTilt {
    previous: Option<(u16, u16)>,
    heading: Option<f64>,
    lean: f64,
}

impl VirtualTilt {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(
        &mut self,
        x: u16,
        y: u16,
        pressure: f64,
        dt_ms: f64,
        maximum: f64,
    ) -> (i32, i32) {
        let dt = (dt_ms / 1000.0).clamp(0.0001, 0.05);
        if let Some((px, py)) = self.previous {
            let dx = (f64::from(x) - f64::from(px)) / 100.0;
            let dy = (f64::from(y) - f64::from(py)) / 100.0;
            let speed = dx.hypot(dy) / dt;
            if speed >= 2.0 {
                let target = dy.atan2(dx) + std::f64::consts::FRAC_PI_2;
                let old = self.heading.unwrap_or(target);
                // Nearest equivalent undirected angle, with bounded turn rate (360 deg/s).
                let delta = (target - old + std::f64::consts::FRAC_PI_2)
                    .rem_euclid(std::f64::consts::PI)
                    - std::f64::consts::FRAC_PI_2;
                self.heading = Some(
                    old + delta.clamp(-std::f64::consts::TAU * dt, std::f64::consts::TAU * dt),
                );
                let desired = maximum
                    * (0.3
                        + 0.55 * (1.0 - pressure.clamp(0.0, 1.0))
                        + 0.15 * speed / (speed + 80.0));
                self.lean += (1.0 - (-dt / 0.025).exp()) * (desired - self.lean);
            }
            // At rest, hold the last orientation instead of amplifying tiny position noise.
        }
        self.previous = Some((x, y));
        let heading = self.heading.unwrap_or(0.0);
        let tangent = self.lean.clamp(0.0, maximum).to_radians().tan();
        (
            (tangent * heading.cos()).atan().to_degrees().round() as i32,
            (tangent * heading.sin()).atan().to_degrees().round() as i32,
        )
    }
}
