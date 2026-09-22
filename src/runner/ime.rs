//! Ordered IME keystroke ownership, shared by the rendered and semantic
//! input dispatch loops.
//!
//! Platform IMEs mark their own keystrokes by what they emit, and
//! ownership follows the event order inside a batch. A `composing` flag —
//! seeded from the renderer's live composition — becomes true at a
//! non-empty preedit and false at a commit, an empty preedit, or
//! `ImeDisabled`; every key/text event is the IME's while `composing`
//! holds at that point in the sequence. The Enter or Backspace that
//! confirms a composition arrives before its commit — still inside it —
//! and is consumed, while a key arriving after the commit (a shortcut, or
//! plain typing in direct-commit mode) is ordinary input again.
//!
//! Ordering alone cannot place the keystrokes that *produced* a
//! composition event, because platforms report them first: wl_keyboard
//! forwards the raw key — and IBus/fcitx5 the keysym-derived text — inside
//! the same flush as the `zwp_text_input_v3` preedit they generated, and
//! AppKit delivers the `keyDown` before the `insertText` it becomes. So a
//! modifier-free key press or a text event is additionally the IME's when
//! the batch later delivers *any* IME event — that event is the platform's
//! answer to those keystrokes, and suppressing only up to it, never past
//! it, keeps the batch's tail (the post-commit keys) ordinary.
//!
//! Two ordering details decide the ambiguous cases:
//!
//! - A commit with no live composition is ordinary text, not a composition
//!   boundary: fcitx5/IBus in direct-commit mode deliver plain characters
//!   that never mark a preedit, and AppKit routes every unmarked keystroke
//!   through `insertText`. It ends nothing and owns no neighbouring keys —
//!   except the press that produced it: a key immediately followed by a
//!   commit is that keystroke delivered twice (winit reports both the
//!   `KeyboardInput` and the `Ime::Commit` for an unmarked `insertText`),
//!   and the commit carries the authoritative text, so the press stays
//!   inside the IME or the character would be inserted a second time.
//!
//! - A chord (Control/Alt/Super held) never produces text: it is the
//!   IME's only while a composition is live, never by looking ahead — a
//!   commit after it cannot be its own delivery.

use crate::platform::{InputEvent, KeyState};

/// Whether an event is a composition boundary the ownership walk tracks —
/// any `Ime*` variant.
fn is_ime_boundary(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::ImePreedit { .. } | InputEvent::ImeCommit { .. } | InputEvent::ImeDisabled
    )
}

/// Flags, per event in a drained platform batch, whether a key or text
/// event belongs to the IME rather than ordinary input handling.
/// `composing` seeds the walk with the composition the renderer already
/// holds; the returned flags are meaningful for `InputEvent::Key` and
/// `InputEvent::TextInput` entries and `false` for everything else.
pub(crate) fn ime_owned_events(events: &[InputEvent], composing: bool) -> Vec<bool> {
    let mut composing = composing;
    let mut owned = Vec::with_capacity(events.len());
    for (index, event) in events.iter().enumerate() {
        let answered_by_ime = events[index + 1..].iter().any(is_ime_boundary);
        owned.push(match event {
            InputEvent::Key {
                state: KeyState::Pressed,
                modifiers,
                ..
            } => {
                composing
                    || (!modifiers.control
                        && !modifiers.alt
                        && !modifiers.super_key
                        && answered_by_ime)
            }
            InputEvent::Key { .. } | InputEvent::TextInput { .. } => {
                composing || (matches!(event, InputEvent::TextInput { .. }) && answered_by_ime)
            }
            InputEvent::ImePreedit { text, .. } => {
                composing = !text.is_empty();
                false
            }
            InputEvent::ImeCommit { .. } | InputEvent::ImeDisabled => {
                composing = false;
                false
            }
            _ => false,
        });
    }
    owned
}
