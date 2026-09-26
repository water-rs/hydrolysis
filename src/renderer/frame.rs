//! Frame lifecycle: scene reset, rebuild/redraw frame boundaries, layer
//! stack management, frame triggers, and per-frame statistics.

use super::*;

use std::collections::HashMap;
use cherenkov::Draw;

/// CPU and GPU stage times of one pumped frame, for the `frame_profile`
/// example and the headless runner's frame report.
#[cfg(feature = "frame-profile")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStageTimes {
    /// Reactive update / patch application.
    pub update: Duration,
    /// Layout of the retained tree.
    pub layout: Duration,
    /// Scene encoding: recording the retained tree into the frame picture and
    /// committing it to the surface.
    pub encode: Duration,
    /// GPU time the engine reported for the frame, when timestamp queries are
    /// enabled and the adapter supports them.
    pub gpu: Option<Duration>,
    /// Waiting for the engine to render the frame.
    pub gpu_wait: Duration,
    /// Offscreen readback of the rendered frame.
    pub readback: Duration,
}

pub(crate) fn duration_micros_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

/// Whether a scene encodes any visible content.
pub(crate) fn scene_has_content(scene: &crate::scene::Scene) -> bool {
    scene.has_content()
}

/// A clip layer open in the frame scene. Re-pushed onto the fresh scene
/// after a whole-scene flush so the retained walk's clip nesting survives the
/// layer boundary.
#[derive(Clone, Debug)]
pub(crate) struct ActiveSceneLayer {
    pub(crate) alpha: f32,
    pub(crate) transform: cherenkov::kurbo::Affine,
    pub(crate) shape: cherenkov::ShapeData,
}

impl ActiveSceneLayer {
    fn push_to_scene(&self, scene: &mut crate::scene::Scene) {
        scene.push_layer_shape(self.alpha, self.transform, self.shape.clone());
    }
}

impl SemanticCore {
    /// Whether the persistent render tree has been built. The view tree's `body()`
    /// is dispatched recursively exactly once — on the first frame, when this is
    /// `false`. Afterwards every change (reactive value, structural patch, scroll,
    /// resize, interaction) is reflected by refreshing this retained tree, so the
    /// runner routes any later rebuild request through the refresh pump instead of
    /// re-running `build_content`.
    #[must_use]
    pub fn has_render_tree(&self) -> bool {
        self.render_tree.is_some()
    }

    #[must_use]
    pub fn state(&self) -> &HydroState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut HydroState {
        &mut self.state
    }

    /// Drop cached `Dynamic` measurements whose node has left the retained tree.
    ///
    /// Shared by the two frames that can remove nodes: the one-time build and any
    /// refresh that applied a structural patch.
    pub(crate) fn prune_dynamic_measurements(&mut self, live: &FxHashSet<usize>) {
        self.state
            .measurement
            .retain_dynamic_identities(|identity| live.contains(&identity));
    }

    /// Drop text-input focus / the selection drag when the target they name is
    /// no longer emitted. Targets are pure-emission (rebuilt in flush order every
    /// frame), so after any flush — rebuild or refresh — a previously focused
    /// field may be gone. Both are held by stable identity, so this only asks
    /// whether that identity still resolves; it can never mistake a different
    /// field that moved into the old position for the focused one. Shared by both
    /// frame paths.
    pub(crate) fn validate_focused_text_input_after_flush(&mut self) {
        let modal_active = self.hit_test.modal_interaction.is_some();
        let focus_is_live = !self.text_editing.has_focus()
            || self
                .text_editing
                .focused_target()
                .is_some_and(|target| !modal_active || target.modal);
        if !focus_is_live {
            self.set_focused_text_input_key(None);
        }
        // The semantic focus node must still be emitted and hittable: a node
        // that went hidden or disabled — itself or through an ancestor —
        // releases focus rather than keeping it on an inert view.
        #[cfg(feature = "accessibility")]
        {
            if !self
                .keyboard_focus_node()
                .is_none_or(|node| self.emitted_node_is_live(node))
            {
                self.hit_test.focus_dropped_this_frame = true;
                self.set_keyboard_focus_node(None, false);
            }
        }
        let keyboard_focus_is_live =
            self.hit_test.keyboard_focus.as_ref().is_none_or(|focused| {
                self.hit_test.pointer_targets.iter().any(|target| {
                    (!modal_active || target.modal)
                        && target
                            .press_slot
                            .as_ref()
                            .is_some_and(|slot| &slot.key == focused)
                }) || self.text_editing.text_input_targets.iter().any(|target| {
                    (!modal_active || target.modal) && &target.interaction_key == focused
                }) || self
                    .hit_test
                    .embedded_input_targets
                    .iter()
                    .any(|target| &target.interaction_key == focused)
                    || {
                        // A key resolved through the semantic focus link stays
                        // live while its node emits — the semantic walk emits
                        // no pointer machinery to back a key. In the rendered
                        // runtime a key backed by no target is dead: its widget
                        // went non-hittable and the emitted node cannot
                        // resurrect it.
                        #[cfg(feature = "accessibility")]
                        {
                            self.semantic_walk
                                && self
                                    .focus_node_for_key(focused)
                                    .is_some_and(|node| self.emitted_node_is_live(node))
                        }
                        #[cfg(not(feature = "accessibility"))]
                        {
                            false
                        }
                    }
            });
        if !keyboard_focus_is_live {
            self.hit_test.focus_dropped_this_frame = true;
            self.set_keyboard_focus(None, false);
        }
        let selection_drag_is_live = self
            .text_editing
            .active_text_selection_drag
            .as_ref()
            .is_none_or(|drag| self.text_editing.index_of(&drag.target).is_some());
        if !selection_drag_is_live {
            self.text_editing.active_text_selection_drag = None;
        }
    }

    /// Relocates focus that lost its view to this frame's flush: a hidden
    /// or unmounted view cannot keep focus, and dropping it on the floor
    /// leaves every keystroke dead until a pointer press re-grants it —
    /// the #126 regression. Focus relocates to the next focusable after the
    /// freed slot, in the emitted tree order `traversal_anchor` tracks —
    /// the same move Tab traversal performs. A `.focused(binding)` grant
    /// that landed in the same frame wins over the relocation, as does any
    /// focusable the transition already handed focus to.
    pub(crate) fn relocate_dropped_focus(&mut self) {
        if !self.hit_test.focus_dropped_this_frame {
            return;
        }
        self.hit_test.focus_dropped_this_frame = false;
        let focus_alive = self.hit_test.focused_embedded_key.is_some()
            || self.hit_test.keyboard_focus.is_some()
            || {
                #[cfg(feature = "accessibility")]
                {
                    self.keyboard_focus_node().is_some()
                }
                #[cfg(not(feature = "accessibility"))]
                {
                    false
                }
            };
        if !focus_alive {
            self.move_keyboard_focus(false);
        }
    }

    /// Whether `node` was emitted this frame and still accepts focus: not
    /// hidden and not disabled, itself or through an ancestor's state.
    #[cfg(feature = "accessibility")]
    pub(crate) fn emitted_node_is_live(&self, node: AccessibilityNodeId) -> bool {
        self.accessibility
            .nodes
            .iter()
            .any(|(id, emitted)| *id == node && !emitted.is_hidden() && !emitted.is_disabled())
    }

    /// The shared frame-trigger handle for closures that outlive a borrow of
    /// the renderer (navigation controllers, GPU-surface invalidators, …).
    pub(crate) fn frame_signals(&self) -> FrameSignals {
        self.signals.clone()
    }

    pub fn request_redraw(&self) {
        self.signals.request_redraw();
    }

    /// Schedules a full frame: every awake frame re-reads signals, runs
    /// layout, and re-encodes the retained tree. Reactive updates and visual
    /// values outside the reactive graph (a scroll offset, a scrollbar drag)
    /// share this one path.
    pub fn request_refresh(&self) {
        self.signals.request_refresh();
    }

    pub fn take_redraw_request(&self) -> bool {
        self.signals.take_redraw_request()
    }

    pub fn request_rebuild(&self) {
        self.signals.request_rebuild();
    }

    #[must_use]
    pub fn has_rebuild_request(&self) -> bool {
        self.signals.has_rebuild_request()
    }

    pub fn request_next_frame_rebuild(&self) {
        self.signals.request_next_frame_rebuild();
    }

    pub fn take_rebuild_request(&self) -> bool {
        self.signals.take_rebuild_request()
    }

    #[must_use]
    pub fn has_patch_request(&self) -> bool {
        self.signals.has_patch_request()
    }

    pub fn take_patch_request(&self) -> bool {
        self.signals.take_patch_request()
    }

    pub fn take_next_frame_rebuild_request(&self) -> bool {
        self.signals.take_next_frame_rebuild_request()
    }

    /// Whether a state change has already been requested but not yet applied,
    /// so the semantics the last flush produced are stale.
    ///
    /// This is the *unapplied* half of [`Self::has_scheduled_semantic_work`]:
    /// a signal fired and asked for a patch or a rebuild, and the next flush
    /// will show a different tree. It deliberately excludes work that merely
    /// continues over future frames — animations, gesture deadlines, gliding
    /// scrolls — because those never stop asking, so a caller that waits on
    /// them waits forever. An observer that needs to see the current state
    /// waits on this; one that needs the app to come fully to rest waits on
    /// `has_scheduled_semantic_work`.
    #[must_use]
    pub fn has_pending_semantic_update(&self) -> bool {
        self.signals.has_patch_request()
            || self.signals.has_rebuild_request()
            || self.signals.has_next_frame_rebuild_request()
    }

    /// Whether the renderer has scheduled work that will still change layout,
    /// semantics, or reactive state on a future frame: pending patches or
    /// rebuilds, active animations, armed gesture deadlines, or gliding smooth
    /// scrolls.
    ///
    /// Visual-only redraw requests (caret blink, the visible-window present
    /// cadence) are deliberately excluded: they repaint pixels without moving
    /// semantic state, and a focused text caret blinks forever — including it
    /// would make an app with a focused field never count as settled.
    #[must_use]
    pub fn has_scheduled_semantic_work(&self) -> bool {
        self.has_pending_semantic_update()
            || self.animations_active()
            || self.next_gesture_deadline().is_some()
            || self.has_gliding_smooth_scrolls()
    }

    pub(crate) fn measurement_cache_stats(&self) -> (u32, u32) {
        self.state.measurement.stats()
    }
}

impl HydrolysisRenderer {
    /// Records the window's logical bounds and the root transform that maps
    /// them onto the target's physical pixel grid.
    pub(crate) fn set_window_viewport(
        &mut self,
        bounds: cherenkov::kurbo::Rect,
        root_transform: cherenkov::kurbo::Affine,
    ) {
        self.window_bounds = bounds;
        self.window_root_transform = root_transform;
    }

    /// The window's viewport in physical pixels: where the root transform puts
    /// the window's logical bounds.
    pub(crate) fn window_viewport(&self) -> cherenkov::kurbo::Rect {
        self.window_root_transform
            .transform_rect_bbox(self.window_bounds)
    }

    pub(crate) fn state_and_scene_mut(&mut self) -> (&mut HydroState, &mut crate::scene::Scene) {
        (&mut self.core.state, &mut self.scene)
    }

    #[must_use]
    pub fn scene(&self) -> &crate::scene::Scene {
        &self.scene
    }

    pub fn reset_scene(&mut self) {
        self.hit_test.reset_scene();
        self.gesture_engine.clear_targets();
        self.text_editing.text_input_targets.clear();
        self.scene.reset();
        self.frame_pictures.clear();
        self.frame_recorded = true;
        self.active_scene_layers.clear();
        self.state.measurement.reset_counters();
        self.frame_clip_layers = 0;
        self.frame_max_clip_depth = 0;
        #[cfg(feature = "accessibility")]
        self.accessibility.reset_scene();
    }

    pub fn begin_rebuild_frame(&mut self) {
        // A full rebuild re-dispatches every Dynamic node, so any pending isolated
        // reactive patch is subsumed by it.
        self.signals.begin_rebuild();
        self.state.measurement.begin_frame();
        self.frame_clip_layers = 0;
        self.frame_max_clip_depth = 0;
        self.lifecycle.begin_rebuild_frame();
        self.hit_test.begin_rebuild_frame();
        self.gesture_group_ids.clear();
        self.next_gesture_group_id = 0;
        self.animation_controller.begin_rebuild_frame();
        self.lazy.begin_rebuild_frame();
        self.navigation.begin_rebuild_frame();
        self.frame_pictures.clear();
        self.frame_recorded = true;
        self.active_scene_layers.clear();
        #[cfg(feature = "accessibility")]
        self.accessibility.begin_rebuild_frame();
    }

    pub(crate) fn begin_redraw_frame(&mut self) {
        // Clear the per-frame `stable_ptr`-keyed view-dimension cache, not just the
        // counters: the refresh path runs full layout every frame, so it measures
        // `RetainedSubview`/widget content through that cache. Its keys are view heap
        // addresses, unique only within a frame (a freed view's address is reused next
        // frame), so a stale entry would otherwise be read as a different view's size.
        self.state.measurement.begin_frame();
        self.frame_clip_layers = 0;
        self.frame_max_clip_depth = 0;
    }

    pub fn finish_rebuild_frame(&mut self) {
        assert!(
            self.active_scene_layers.is_empty(),
            "hydrolysis renderer: scene layer stack must be empty at end of rebuild (len={})",
            self.active_scene_layers.len()
        );
        self.flush_scene_layer();
        self.lifecycle.finish_rebuild_frame();
        let live_dynamics = self
            .render_tree
            .as_ref()
            .map(RenderNode::collect_dynamic_identities)
            .unwrap_or_default();
        self.prune_dynamic_measurements(&live_dynamics);

        self.validate_focused_text_input_after_flush();

        self.core
            .animation_controller
            .finish_rebuild_frame_with_inactive_slot_retention(false);
        self.core
            .hit_test
            .finish_rebuild_frame(&self.core.text_editing.text_input_targets);
        self.relocate_dropped_focus();
        self.core.navigation.finish_rebuild_frame();
        self.core.signals.finish_rebuild();
        #[cfg(feature = "accessibility")]
        self.finalize_accessibility_tree_update();
    }

    pub fn scene_mut(&mut self) -> &mut crate::scene::Scene {
        &mut self.scene
    }

    pub(crate) fn push_layer_rect(
        &mut self,
        alpha: f32,
        transform: cherenkov::kurbo::Affine,
        rect: cherenkov::kurbo::Rect,
    ) {
        self.push_layer_shape(alpha, transform, cherenkov::ShapeData::Rect(rect));
    }

    pub(crate) fn push_layer_shape(
        &mut self,
        alpha: f32,
        transform: cherenkov::kurbo::Affine,
        shape: cherenkov::ShapeData,
    ) {
        self.record_clip_layer_push();
        self.scene.push_layer_shape(alpha, transform, shape.clone());
        self.active_scene_layers.push(ActiveSceneLayer {
            alpha,
            transform,
            shape,
        });
    }

    pub(crate) fn pop_layer(&mut self) {
        self.scene.pop_layer();
        self.active_scene_layers
            .pop()
            .expect("hydrolysis renderer: pop_layer underflow");
    }

    pub(super) fn record_clip_layer_push(&mut self) {
        self.frame_clip_layers = self
            .frame_clip_layers
            .checked_add(1)
            .expect("hydrolysis frame clip layer counter overflow");
        let depth = u32::try_from(self.active_scene_layers.len() + 1)
            .expect("hydrolysis active scene layer depth exceeds u32");
        self.frame_max_clip_depth = self.frame_max_clip_depth.max(depth);
    }

    /// Closes the frame scene into a picture: the open clip layers are popped,
    /// the scene is recorded and queued for the frame, and the clip layers are
    /// re-pushed onto the fresh scene so the retained walk continues inside
    /// them.
    pub(super) fn flush_scene_layer(&mut self) {
        assert!(
            self.scene.open_layers() == self.active_scene_layers.len(),
            "hydrolysis renderer: scene clip count {} does not match tracked scene layers {}",
            self.scene.open_layers(),
            self.active_scene_layers.len()
        );

        for _ in 0..self.active_scene_layers.len() {
            self.scene.pop_layer();
        }

        if self.scene.has_content() {
            let scene = core::mem::take(&mut self.scene);
            self.frame_pictures.push(scene.to_picture());
        } else {
            self.scene.reset();
        }

        for layer in &self.active_scene_layers {
            layer.push_to_scene(&mut self.scene);
        }
    }

    /// Takes the frame's whole-scene picture — every flushed segment appended
    /// in order — or `None` when the scene was not re-recorded since the last
    /// frame and the root layer keeps its retained picture.
    pub(crate) fn take_frame_picture(&mut self) -> Option<cherenkov::Picture> {
        if !self.frame_recorded {
            return None;
        }
        self.frame_recorded = false;
        let pictures = core::mem::take(&mut self.frame_pictures);
        self.frame_picture_count =
            u32::try_from(pictures.len()).expect("frame picture count fits u32");
        if pictures.len() == 1 {
            return pictures.into_iter().next();
        }
        Some(cherenkov::Picture::record(|recorder| {
            for picture in &pictures {
                recorder.picture(picture, cherenkov::kurbo::Affine::IDENTITY);
            }
        }))
    }

    /// Installs the frame's whole-scene picture on `surface`'s root layer and
    /// renders every surface of the engine at `now`.
    ///
    /// Returns when the engine next wants a frame; the runner folds it into
    /// its pump. The pump keeps `Idle`/`Refresh`: a settled scene leaves the
    /// engine idle and no frame is submitted until content changes.
    ///
    /// # Errors
    /// The engine's render error; the runner maps it to a surface loss.
    pub fn present_frame(
        &mut self,
        surface: &cherenkov::Surface<cherenkov_gpu::Gpu>,
        clear_color: cherenkov::WorkingColor,
        now: Instant,
    ) -> Result<cherenkov::Next, cherenkov::RenderError> {
        let picture = self.take_frame_picture();
        let overlay = self
            .transient_scene
            .take()
            .filter(crate::scene::Scene::has_content)
            .map(|scene| scene.to_picture());
        let attach_overlay = overlay.is_some() && self.overlay_layer.is_none();
        let overlay_layer = self.overlay_layer.get_or_insert_with(|| surface.layer());
        surface.clear_color(clear_color);
        surface.update(|tx| {
            if let Some(picture) = picture {
                tx[surface.root()].content(picture);
            }
            if attach_overlay {
                tx[surface.root()].push(overlay_layer);
            }
            match overlay {
                Some(overlay) => tx[&*overlay_layer].content(overlay),
                None => tx[&*overlay_layer].clear_content(),
            };
        });
        #[cfg(feature = "frame-profile")]
        let started = Instant::now();
        let next = self.engine.render(cherenkov::FrameTime::at(now))?;
        #[cfg(feature = "frame-profile")]
        {
            self.frame_stage_times.gpu_wait = started.elapsed();
        }
        Ok(next)
    }

    /// Segments in the last frame picture.
    pub(crate) fn frame_picture_count(&self) -> u32 {
        self.frame_picture_count
    }

    pub(crate) fn clip_layer_stats(&self) -> (u32, u32) {
        (self.frame_clip_layers, self.frame_max_clip_depth)
    }

    /// Drains the stage times accumulated since the last call.
    #[cfg(feature = "frame-profile")]
    pub fn take_frame_stage_times(&mut self) -> FrameStageTimes {
        core::mem::take(&mut self.frame_stage_times)
    }

    /// Digest of the last layout pass's placed bounds.
    #[cfg(feature = "frame-profile")]
    #[must_use]
    pub fn layout_signature(&self) -> Option<u64> {
        self.last_layout_signature
    }
}
