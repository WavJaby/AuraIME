use std::time::{Duration, Instant};

pub const FADE_IN_DURATION: Duration = Duration::from_millis(100);
pub const SHOW_DURATION: Duration = Duration::from_millis(800);
pub const FADE_OUT_DURATION: Duration = Duration::from_millis(100);
pub const ANIM_OFFSET: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationPhase {
    FadeIn,
    Visible,
    FadeOut,
    Finished,
}

pub struct AnimationState {
    pub start_time: Option<Instant>,
}

impl AnimationState {
    pub fn new() -> Self {
        Self { start_time: None }
    }

    pub fn on_activity(&mut self) -> bool {
        let now = Instant::now();
        let mut skip_fade_in = false;
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed();
            // If it's already in "Fully visible" phase, keep it there by shifting start_time
            if elapsed >= FADE_IN_DURATION && elapsed < FADE_IN_DURATION + SHOW_DURATION {
                self.start_time = Some(now - FADE_IN_DURATION);
                skip_fade_in = true;
            } else if elapsed < FADE_IN_DURATION {
                // If fading in, don't reset to 0, just stay in fade-in
                skip_fade_in = true;
            } else {
                self.start_time = Some(now);
            }
        } else {
            self.start_time = Some(now);
        }
        skip_fade_in
    }

    pub fn get_phase(&self) -> AnimationPhase {
        let start = match self.start_time {
            Some(s) => s,
            None => return AnimationPhase::Finished,
        };

        let elapsed = start.elapsed();
        if elapsed < FADE_IN_DURATION {
            AnimationPhase::FadeIn
        } else if elapsed < FADE_IN_DURATION + SHOW_DURATION {
            AnimationPhase::Visible
        } else if elapsed < FADE_IN_DURATION + SHOW_DURATION + FADE_OUT_DURATION {
            AnimationPhase::FadeOut
        } else {
            AnimationPhase::Finished
        }
    }

    pub fn get_alpha_and_offset(&self) -> (f32, f32) {
        let start = match self.start_time {
            Some(s) => s,
            None => return (0.0, ANIM_OFFSET),
        };

        let elapsed = start.elapsed();
        if elapsed < FADE_IN_DURATION {
            let t = elapsed.as_secs_f32() / FADE_IN_DURATION.as_secs_f32();
            (t, ANIM_OFFSET * (1.0 - t))
        } else if elapsed < FADE_IN_DURATION + SHOW_DURATION {
            (1.0, 0.0)
        } else if elapsed < FADE_IN_DURATION + SHOW_DURATION + FADE_OUT_DURATION {
            let t = (elapsed - (FADE_IN_DURATION + SHOW_DURATION)).as_secs_f32() / FADE_OUT_DURATION.as_secs_f32();
            (1.0 - t, ANIM_OFFSET * t)
        } else {
            (0.0, ANIM_OFFSET)
        }
    }
}
