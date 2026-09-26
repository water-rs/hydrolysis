//! Animated-scalar resolution and morph-progress sampling for the retained render
//! tree. These re-sample animated transform/opacity/morph signals every flush so
//! the node tree's transform/opacity/morph nodes stay live without re-dispatching.

use super::signals::SubscribedSnapshot;
use super::*;

pub(crate) fn affine_near(left: cherenkov::kurbo::Affine, right: cherenkov::kurbo::Affine) -> bool {
    left.as_coeffs()
        .iter()
        .zip(right.as_coeffs())
        .all(|(left, right)| (*left - right).abs() <= 0.001)
}

pub(crate) fn rect_near(left: cherenkov::kurbo::Rect, right: cherenkov::kurbo::Rect) -> bool {
    (left.x0 - right.x0).abs() <= 0.001
        && (left.y0 - right.y0).abs() <= 0.001
        && (left.x1 - right.x1).abs() <= 0.001
        && (left.y1 - right.y1).abs() <= 0.001
}

impl HydrolysisRenderer {
    #[cfg(test)]
    pub(crate) fn scene_is_empty(&self) -> bool {
        !scene_has_content(&self.scene)
    }
}

impl SemanticCore {
    pub(super) fn resolve_animated_scalar_with_discriminator<S>(
        &mut self,
        signal: &S,
        discriminator: usize,
    ) -> f32
    where
        S: Signal<Output = f32> + Clone + 'static,
    {
        let Some(identity) = signal.identity() else {
            return self.read_signal(signal);
        };
        let now = self.frame_instant;
        let key = AnimationKey::scalar_with_discriminator(identity, discriminator);
        let (subscription, observed_value) = SubscribedSnapshot::new(signal);
        let handle = self
            .animation_controller
            .bind_scalar(key, observed_value, now);
        let watcher_handle = handle.clone();
        let signals = self.signals.clone();
        let guard = subscription.activate(move |update| {
            watcher_handle.apply_update_from_context(update, signals.frame_clock());
            signals.request_redraw();
        });
        self.lifecycle.current_frame_retain.push(Retain::new(guard));
        handle.sample(now)
    }

    /// Sample a time-based shape-morph phase. `node_id` is the stable identity of
    /// the owning morph node (its retained `Rc` address), so the timeline slot keys
    /// off node identity and survives across frames and structural changes — unlike a
    /// positional `render_depth`, which shifts when a sibling subtree's node count
    /// changes and would restart the morph mid-animation.
    pub(crate) fn sample_morph_progress(
        &mut self,
        animation: waterui_shape::MorphAnimation,
        node_id: usize,
    ) -> f32 {
        let duration = animation.curve.duration;
        if duration.is_zero() {
            return 1.0;
        }
        let key = AnimationKey::renderer_local_repeating(node_id);
        let elapsed = self.animation_controller.bind_timeline_phase(
            key,
            duration,
            animation.repeat,
            self.frame_instant,
        );
        let raw = elapsed.as_secs_f32() / duration.as_secs_f32();
        let cycle = if animation.repeat {
            let base = raw.fract();
            assert!(
                raw.is_finite() && raw >= 0.0,
                "morph animation cycle index must be finite and non-negative"
            );
            let index = raw.floor() as u64;
            if animation.autoreverse && index % 2 == 1 {
                1.0 - base
            } else {
                base
            }
        } else {
            raw.clamp(0.0, 1.0)
        };
        cherenkov::curve_value(&animation.curve, f64::from(cycle)) as f32
    }
}

/// The settled-fraction of a time-based transition `elapsed` after it started:
/// `1` once the animation has completed. Layout-affecting transitions (a
/// collection entry collapsing along its stack axis) cannot be layer
/// properties, so they sample the animation on the frame clock.
///
/// # Panics
/// A `Decay` only drives `scroll_offset`; it is not a transition animation.
pub(crate) fn transition_progress(animation: &cherenkov::Animation, elapsed: Duration) -> f32 {
    match animation {
        cherenkov::Animation::Curve(curve) => {
            if curve.duration.is_zero() {
                return 1.0;
            }
            let t = elapsed.as_secs_f64() / curve.duration.as_secs_f64();
            cherenkov::curve_value(curve, t) as f32
        }
        cherenkov::Animation::Spring(spring) => {
            let ([position], [velocity]) =
                cherenkov::spring_step([0.0], [0.0], [1.0], spring, elapsed.as_secs_f64());
            if cherenkov::settled([position], [velocity], [1.0]) {
                1.0
            } else {
                position as f32
            }
        }
        cherenkov::Animation::Decay(_) => {
            panic!("hydrolysis: a Decay animation only drives scroll_offset, not a transition")
        }
    }
}

/// Whether a transition under `animation` has finished `elapsed` after it
/// started.
pub(crate) fn transition_complete(animation: &cherenkov::Animation, elapsed: Duration) -> bool {
    match animation {
        cherenkov::Animation::Curve(curve) => elapsed >= curve.duration,
        cherenkov::Animation::Spring(spring) => {
            let ([position], [velocity]) =
                cherenkov::spring_step([0.0], [0.0], [1.0], spring, elapsed.as_secs_f64());
            cherenkov::settled([position], [velocity], [1.0])
        }
        cherenkov::Animation::Decay(_) => {
            panic!("hydrolysis: a Decay animation only drives scroll_offset, not a transition")
        }
    }
}
