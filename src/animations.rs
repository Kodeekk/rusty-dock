use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationState {
    Idle,
    Running,
    Completed,
}

#[derive(Debug, Clone)]
pub struct Animation {
    start_time: Instant,
    duration: Duration,
    start_value: f64,
    end_value: f64,
    state: AnimationState,
}

impl Animation {
    pub fn new(start_value: f64, end_value: f64, duration_ms: u64) -> Self {
        Self {
            start_time: Instant::now(),
            duration: Duration::from_millis(duration_ms),
            start_value,
            end_value,
            state: AnimationState::Running,
        }
    }

    pub fn current_value(&mut self) -> f64 {
        if self.state == AnimationState::Completed {
            return self.end_value;
        }

        let elapsed = self.start_time.elapsed();

        if elapsed >= self.duration {
            self.state = AnimationState::Completed;
            return self.end_value;
        }

        let progress = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        let eased_progress = self.ease_out_cubic(progress);

        self.start_value + (self.end_value - self.start_value) * eased_progress
    }

    pub fn is_running(&self) -> bool {
        self.state == AnimationState::Running
    }

    fn ease_out_cubic(&self, t: f64) -> f64 {
        let t = t - 1.0;
        t * t * t + 1.0
    }
}

#[derive(Debug, Clone)]
pub struct BounceAnimation {
    start_time: Instant,
    duration: Duration,
    bounce_height: f64,
    state: AnimationState,
}

impl BounceAnimation {
    pub fn new(bounce_height: f64, duration_ms: u64) -> Self {
        Self {
            start_time: Instant::now(),
            duration: Duration::from_millis(duration_ms),
            bounce_height,
            state: AnimationState::Running,
        }
    }

    pub fn current_offset(&mut self) -> f64 {
        if self.state == AnimationState::Completed {
            return 0.0;
        }

        let elapsed = self.start_time.elapsed();

        if elapsed >= self.duration {
            self.state = AnimationState::Completed;
            return 0.0;
        }

        let progress = elapsed.as_secs_f64() / self.duration.as_secs_f64();

        // Bounce curve: goes up then down with elastic effect
        let bounce = (1.0 - progress) * (progress * std::f64::consts::PI * 2.0).sin();

        -bounce * self.bounce_height
    }

    pub fn is_running(&self) -> bool {
        self.state == AnimationState::Running
    }
}

pub struct DockAnimations {
    pub visibility: Option<Animation>,
    pub icon_scales: Vec<Option<Animation>>,
    pub icon_bounces: Vec<Option<BounceAnimation>>,
    pub folder_popup: Option<Animation>,  // Scale animation for folder popup
}

impl DockAnimations {
    pub fn new() -> Self {
        Self {
            visibility: None,
            icon_scales: Vec::new(),
            icon_bounces: Vec::new(),
            folder_popup: None,
        }
    }

    pub fn ensure_capacity(&mut self, count: usize) {
        while self.icon_scales.len() < count {
            self.icon_scales.push(None);
            self.icon_bounces.push(None);
        }
    }

    pub fn start_visibility_animation(&mut self, target_visible: bool, duration_ms: u64) {
        let current = self.get_visibility();
        let target = if target_visible { 1.0 } else { 0.0 };

        if (current - target).abs() > 0.01 {
            self.visibility = Some(Animation::new(current, target, duration_ms));
        }
    }

    pub fn start_icon_scale(&mut self, index: usize, target_scale: f64, duration_ms: u64) {
        self.ensure_capacity(index + 1);

        let current = self.get_icon_scale(index);
        if (current - target_scale).abs() > 0.01 {
            self.icon_scales[index] = Some(Animation::new(current, target_scale, duration_ms));
        }
    }

    pub fn start_bounce(&mut self, index: usize, bounce_height: f64, duration_ms: u64) {
        self.ensure_capacity(index + 1);
        self.icon_bounces[index] = Some(BounceAnimation::new(bounce_height, duration_ms));
    }

    pub fn get_visibility(&mut self) -> f64 {
        if let Some(anim) = &mut self.visibility {
            anim.current_value()
        } else {
            1.0
        }
    }

    pub fn get_icon_scale(&mut self, index: usize) -> f64 {
        if index < self.icon_scales.len() {
            if let Some(anim) = &mut self.icon_scales[index] {
                return anim.current_value();
            }
        }
        1.0
    }

    pub fn get_bounce_offset(&mut self, index: usize) -> f64 {
        if index < self.icon_bounces.len() {
            if let Some(anim) = &mut self.icon_bounces[index] {
                return anim.current_offset();
            }
        }
        0.0
    }

    pub fn is_animating(&self) -> bool {
        if let Some(anim) = &self.visibility {
            if anim.is_running() {
                return true;
            }
        }

        for anim in &self.icon_scales {
            if let Some(a) = anim {
                if a.is_running() {
                    return true;
                }
            }
        }

        for anim in &self.icon_bounces {
            if let Some(a) = anim {
                if a.is_running() {
                    return true;
                }
            }
        }

        if let Some(anim) = &self.folder_popup {
            if anim.is_running() {
                return true;
            }
        }

        false
    }

    pub fn start_folder_popup(&mut self, open: bool, duration_ms: u64) {
        let current = self.get_folder_popup_scale();
        let target = if open { 1.0 } else { 0.0 };
        if (current - target).abs() > 0.01 {
            self.folder_popup = Some(Animation::new(current, target, duration_ms));
        }
    }

    pub fn get_folder_popup_scale(&mut self) -> f64 {
        if let Some(anim) = &mut self.folder_popup {
            anim.current_value()
        } else {
            1.0
        }
    }
}