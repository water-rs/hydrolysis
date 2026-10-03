//! The retained state a `GpuContentView` leaf carries through the frame.
//!
//! `GpuContentView` owns its producer — a `Send` `GpuContent` the engine runs
//! on its render thread — plus the UI-side hooks (input, per-frame pump, ime
//! caret, accessibility label) that stay on this thread. The compositor holds
//! the view behind this node so input routing and the per-frame
//! [`waterui_graphics::gpu::GpuContentView::frame`] pump keep working after
//! the producer moves to the engine.
//!
//! The producer installs once per `SceneResources` table:
//! [`GpuContentView::take_engine_content`] hands the engine a `GpuContentBox`
//! over a shareable `GpuContentHandle`, so a fresh table — a new window after
//! device loss — re-installs a fresh handle over the same content and keeps
//! the state it accumulated, which is exactly what `engine_content` is for. A
//! keyed mount presenting `GpuContent` lives for the frame's whole key set,
//! and transient (capture) windows never install it: a capture renders
//! through its window's own table and must not consume the live mount's
//! install.

use waterui_graphics::gpu::{ExternalFrameView, FrameReceiver, GpuContentView};

/// The `GpuContentView` a [`crate::renderer::tree::GpuContentNode`] owns, and
/// whether its producer has been installed on an engine layer yet.
///
/// `installed` flips when the first `GpuContentLayer` carrying this runtime
/// reaches a persistent window's install pass — never on a transient target,
/// which would spend the view's single install on a surface that dies with
/// the call.
pub(crate) struct GpuContentRuntime {
    pub(crate) view: GpuContentView,
    /// `true` once `take_engine_content` has run on the current table; the
    /// producer is on the engine from then on and only `gpu_content_size`/
    /// transform edits apply until the table changes.
    pub(crate) installed: bool,
    /// The `SceneResources` table this runtime installed against; see
    /// [`TableAssociation`](crate::renderer::recording::TableAssociation).
    /// A fresh table — a new `CherenkovWindow` after device loss — makes the
    /// install stale, so the compositor clears `installed` on the change and
    /// `engine_content` answers a fresh handle over the same content on the
    /// new device (water-rs/hydrolysis#350).
    pub(crate) recorded_table: crate::renderer::recording::TableAssociation,
}

impl GpuContentRuntime {
    pub(crate) fn new(view: GpuContentView) -> Self {
        Self {
            view,
            installed: false,
            recorded_table: Default::default(),
        }
    }
}

/// The `ExternalFrameView` a [`crate::renderer::tree::ExternalFrameNode`]
/// owns, and the stream's frame receiver once a mount has started it.
///
/// Unlike `GpuContent`, an external-frame source is restartable: the view
/// hands out a fresh [`ExternalFrameStream`] handle every call, and a lost
/// device or a reborn mount starts the source again with the new output.
/// `receiver` is `Some` once the first `ExternalFrameLayer` carrying this
/// runtime has started the source on the window's device.
pub(crate) struct ExternalFrameRuntime {
    pub(crate) view: ExternalFrameView,
    /// The mailbox drain end, installed by the compositor's install pass.
    pub(crate) receiver: Option<FrameReceiver>,
    /// The plane size of the last presented frame, for the stretch transform.
    pub(crate) frame_pixels: Option<(u32, u32)>,
    /// The `SceneResources` table this runtime started the stream against;
    /// see [`TableAssociation`](crate::renderer::recording::TableAssociation).
    /// The receiver and the frames it drains were minted on that table's
    /// device — a fresh table after device loss makes them stale, so the
    /// compositor drops them on the change and the stream restarts on the
    /// live device (water-rs/hydrolysis#350).
    pub(crate) recorded_table: crate::renderer::recording::TableAssociation,
}

impl ExternalFrameRuntime {
    pub(crate) fn new(view: ExternalFrameView) -> Self {
        Self {
            view,
            receiver: None,
            frame_pixels: None,
            recorded_table: Default::default(),
        }
    }
}
