//! The accessibility snapshot published to the Kotlin host.
//!
//! The renderer's merged `accesskit::TreeUpdate` is serialized once per
//! change and pushed to `HydrolysisAccessibilityProvider`, which serves
//! `AccessibilityNodeInfo` for explore-by-touch without an invisible view
//! tree. Actions the provider dispatches come back through
//! `nativeAccessibilityAction` and run through
//! `handle_accessibility_action` — the same code path desktop uses.

use super::jni::JniError;

/// The host's copy of the accessibility tree — whole-tree publishes only;
/// `accesskit` updates already carry the minimal node deltas.
#[derive(Default)]
pub(crate) struct AccessibilitySnapshot {
    /// Serialized `accesskit::TreeUpdate` JSON for the provider.
    #[cfg(feature = "accessibility")]
    tree_json: Option<String>,
    #[cfg(feature = "accessibility")]
    dirty: bool,
}

impl AccessibilitySnapshot {
    /// The latest published tree, consumed by `nativeAccessibilityTree`.
    #[cfg(feature = "accessibility")]
    pub(crate) fn take_json(&mut self) -> Option<&str> {
        if self.dirty {
            self.dirty = false;
            self.tree_json.as_deref()
        } else {
            None
        }
    }
}

/// Publishes a changed accessibility tree to the host. Called at the end of
/// the frame transaction so one frame produces at most one snapshot.
#[cfg(feature = "accessibility")]
pub(crate) fn publish_if_pending(session: &mut super::host::AndroidSession) {
    // Popups have no second band on Android — the merged iterator is empty.
    let Some(update) = session
        .runtime
        .renderer
        .take_merged_accessibility_tree_update(std::iter::empty::<
            &mut crate::renderer::SemanticCore,
        >())
    else {
        return;
    };
    match serde_json::to_string(&update) {
        Ok(json) => {
            session.a11y.tree_json = Some(json);
            session.a11y.dirty = true;
            session.runtime.platform.bridge.accessibility_tree_changed();
        }
        Err(error) => {
            tracing::error!(
                target: "waterui::hydrolysis::android",
                %error,
                "accessibility tree serialization failed"
            );
        }
    }
}

/// Without the `accessibility` feature the module compiles to nothing —
/// the crate's existing contract is that this feature is a build-time gate.
#[cfg(not(feature = "accessibility"))]
pub(crate) fn publish_if_pending(_session: &mut super::host::AndroidSession) {}

/// The `accesskit::Action` bitmask index the provider echoes back. The Kotlin
/// side decodes a node's serialized `actions`/`childActions` bitmask and
/// advertises the platform actions each bit implies; a performed action comes
/// back as the same index, so the JNI edge carries no per-platform constants
/// at all. The table is `accesskit`'s declaration order — the index IS the
/// `ActionIndex`.
#[cfg(feature = "accessibility")]
fn map_action(action: i32) -> Result<accesskit::Action, JniError> {
    use accesskit::Action;
    const ACTIONS: &[Action] = &[
        Action::Click,
        Action::Focus,
        Action::Blur,
        Action::Collapse,
        Action::Expand,
        Action::CustomAction,
        Action::Decrement,
        Action::Increment,
        Action::HideTooltip,
        Action::ShowTooltip,
        Action::ReplaceSelectedText,
        Action::ScrollDown,
        Action::ScrollLeft,
        Action::ScrollRight,
        Action::ScrollUp,
        Action::ScrollIntoView,
        Action::ScrollToPoint,
        Action::SetScrollOffset,
        Action::SetTextSelection,
        Action::SetSequentialFocusNavigationStartingPoint,
        Action::SetValue,
        Action::ShowContextMenu,
    ];
    usize::try_from(action)
        .ok()
        .and_then(|index| ACTIONS.get(index))
        .copied()
        .ok_or_else(|| {
            JniError(format!(
                "hydrolysis android: unsupported accessibility action {action}"
            ))
        })
}

/// Routes an action the provider dispatched for `virtual_view_id` (the
/// accesskit `NodeId` value) back into the renderer — inside the frame
/// boundary like any other input.
///
/// The provider sends the data kind the target's role expects: `text` for
/// `SetValue` on an editable node (the whole replacement text — Android's
/// `ACTION_SET_TEXT` replaces the full contents) and `numeric` for `SetValue`
/// on a range node. A request carrying both is a provider bug, so it errors
/// rather than guessing.
#[cfg(feature = "accessibility")]
pub(crate) fn perform_action(
    session: &mut super::host::AndroidSession,
    virtual_view_id: i64,
    action: i32,
    text: Option<String>,
    numeric: Option<f64>,
) -> Result<bool, JniError> {
    use accesskit::{ActionData, ActionRequest, NodeId, TreeId};

    // The action decides which payload channel is meaningful, so a provider
    // that sends both (or the wrong one) errors instead of being guessed at.
    let action = map_action(action)?;
    let data = match (action, text, numeric) {
        (accesskit::Action::CustomAction, None, Some(index)) => {
            Some(ActionData::CustomAction(index as i32))
        }
        (accesskit::Action::SetValue, Some(text), None) => {
            Some(ActionData::Value(text.into_boxed_str()))
        }
        (accesskit::Action::SetValue, None, Some(numeric)) => {
            Some(ActionData::NumericValue(numeric))
        }
        (_, None, None) => None,
        _ => {
            return Err(JniError(format!(
                "hydrolysis android: accessibility action {action:?} carries mismatched data"
            )));
        }
    };
    let request = ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: NodeId(virtual_view_id.max(0) as u64),
        data,
    };
    Ok(session
        .runtime
        .renderer
        .handle_accessibility_action(request, &session.env))
}

/// Without the feature there is no tree to act on.
#[cfg(not(feature = "accessibility"))]
pub(crate) fn perform_action(
    _session: &mut super::host::AndroidSession,
    _virtual_view_id: i64,
    _action: i32,
    _text: Option<String>,
    _numeric: Option<f64>,
) -> Result<bool, JniError> {
    Ok(false)
}
