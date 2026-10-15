use std::time::Duration;

/// The original game advances thirty times per second, regardless of display rate.
pub const TICKS_PER_SECOND: u32 = 30;
const SECOND: u128 = 1_000_000_000;

#[derive(Debug, Default)]
pub struct FixedClock {
    scaled_remainder: u128,
    pub ticks: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameStep {
    pub ticks: u32,
    pub interpolation: f32,
    /// Long stalls are discarded instead of fast-forwarding the game on return.
    pub discarded: Duration,
}

impl FixedClock {
    pub fn advance(&mut self, elapsed: Duration, paused: bool) -> FrameStep {
        if paused {
            self.scaled_remainder = 0;
            return FrameStep {
                ticks: 0,
                interpolation: 0.0,
                discarded: elapsed,
            };
        }
        let accepted = elapsed.min(Duration::from_millis(250));
        self.scaled_remainder += accepted.as_nanos() * u128::from(TICKS_PER_SECOND);
        let ticks = (self.scaled_remainder / SECOND) as u32;
        self.scaled_remainder %= SECOND;
        self.ticks += u64::from(ticks);
        FrameStep {
            ticks,
            interpolation: ((self.scaled_remainder as f64 / SECOND as f64) as f32)
                .min(1.0_f32.next_down()),
            discarded: elapsed - accepted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_rate_never_changes_simulation_speed() {
        for hz in [30, 60, 75, 120, 144, 165, 240] {
            let mut clock = FixedClock::default();
            let mut previous = 0;
            for frame in 1..=hz * 10 {
                let now = frame * 1_000_000_000_u64 / hz;
                let result = clock.advance(Duration::from_nanos(now - previous), false);
                assert!((0.0..1.0).contains(&result.interpolation));
                previous = now;
            }
            assert_eq!(clock.ticks, 300, "{hz} Hz display");
        }
    }

    #[test]
    fn pause_and_stalls_do_not_cause_runaway_catchup() {
        let mut clock = FixedClock::default();
        clock.advance(Duration::from_millis(20), false);
        assert_eq!(clock.advance(Duration::from_secs(20), true).ticks, 0);
        assert_eq!(clock.advance(Duration::from_millis(20), false).ticks, 0);
        let resumed = clock.advance(Duration::from_secs(5), false);
        assert_eq!(resumed.ticks, 8);
        assert_eq!(resumed.discarded, Duration::from_millis(4750));
    }
}
