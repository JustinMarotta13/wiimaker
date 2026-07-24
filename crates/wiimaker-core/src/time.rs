//! Fixed 60 Hz clock — Wii VI cadence is the source of truth.

/// Fixed-timestep clock with an accumulator for host variability.
#[derive(Clone, Debug)]
pub struct Clock {
    pub tick_hz: f32,
    pub dt: f32,
    pub alpha: f32,
    pub frame: u64,
    pub elapsed: f32,
    accumulator: f32,
}

impl Default for Clock {
    fn default() -> Self {
        Self::new(60.0)
    }
}

impl Clock {
    pub fn new(tick_hz: f32) -> Self {
        Self {
            tick_hz,
            dt: 1.0 / tick_hz,
            alpha: 0.0,
            frame: 0,
            elapsed: 0.0,
            accumulator: 0.0,
        }
    }

    /// Push real seconds; returns how many fixed steps to run.
    pub fn push_real(&mut self, real_dt: f32) -> u32 {
        // Clamp to avoid spiral of death after a hitch.
        let real_dt = real_dt.min(0.25);
        self.accumulator += real_dt;
        let mut steps = 0u32;
        while self.accumulator >= self.dt {
            self.accumulator -= self.dt;
            self.elapsed += self.dt;
            self.frame += 1;
            steps += 1;
            if steps > 5 {
                self.accumulator = 0.0;
                break;
            }
        }
        self.alpha = self.accumulator / self.dt;
        steps
    }
}
