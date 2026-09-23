use super::Modifiers;

pub(super) fn should_forward_ime_key(key: &str, modifiers: Modifiers, composing: bool) -> bool {
    if composing || modifiers.alt {
        return false;
    }
    if modifiers.control || modifiers.super_key {
        return !modifiers.shift
            && ["a", "c", "x", "v"]
                .iter()
                .any(|candidate| key.eq_ignore_ascii_case(candidate));
    }
    matches!(
        key,
        "Backspace"
            | "Delete"
            | "ArrowLeft"
            | "ArrowRight"
            | "Home"
            | "End"
            | "Enter"
            | "Tab"
            | "Escape"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        shift: false,
        control: false,
        alt: false,
        super_key: false,
    };

    fn mods(shift: bool, control: bool, alt: bool, super_key: bool) -> Modifiers {
        Modifiers {
            shift,
            control,
            alt,
            super_key,
        }
    }

    #[test]
    fn command_editing_keys_are_forwarded() {
        for command in [
            mods(false, true, false, false),
            mods(false, false, false, true),
        ] {
            for key in ["a", "c", "x", "v"] {
                assert!(should_forward_ime_key(key, command, false));
            }
            assert!(should_forward_ime_key("A", command, false));
        }
    }

    #[test]
    fn plain_editing_and_navigation_keys_are_forwarded() {
        for key in [
            "Backspace",
            "Delete",
            "ArrowLeft",
            "ArrowRight",
            "Home",
            "End",
            "Enter",
            "Tab",
            "Escape",
        ] {
            assert!(should_forward_ime_key(key, NONE, false), "{key}");
        }
    }

    #[test]
    fn shift_selection_keys_are_forwarded() {
        let shift = mods(true, false, false, false);
        for key in ["ArrowLeft", "Home", "Tab"] {
            assert!(should_forward_ime_key(key, shift, false), "{key}");
        }
    }

    #[test]
    fn printable_and_dead_keys_are_not_forwarded() {
        for key in ["a", "1", " ", "你", "Dead"] {
            assert!(!should_forward_ime_key(key, NONE, false), "{key}");
        }
    }

    #[test]
    fn composition_blocks_all_forwarded_keys() {
        for key in ["Backspace", "Delete", "Enter", "Tab", "ArrowLeft", "a"] {
            assert!(
                !should_forward_ime_key(key, NONE, true),
                "{key} while composing"
            );
        }
        assert!(!should_forward_ime_key(
            "a",
            mods(false, true, false, false),
            true
        ));
    }

    #[test]
    fn alt_and_altgr_are_not_forwarded() {
        assert!(!should_forward_ime_key(
            "Backspace",
            mods(false, false, true, false),
            false
        ));
        assert!(!should_forward_ime_key(
            "a",
            mods(false, true, true, false),
            false
        ));
        assert!(!should_forward_ime_key(
            "ArrowLeft",
            mods(false, false, true, false),
            false
        ));
    }

    #[test]
    fn browser_shortcuts_are_not_forwarded() {
        let ctrl = mods(false, true, false, false);
        for key in ["l", "r", "f", "t", "z", "Tab"] {
            assert!(!should_forward_ime_key(key, ctrl, false), "Ctrl+{key}");
        }
        let ctrl_shift = mods(true, true, false, false);
        for key in ["a", "c", "Tab"] {
            assert!(
                !should_forward_ime_key(key, ctrl_shift, false),
                "Ctrl+Shift+{key}"
            );
        }
        let meta_shift = mods(true, false, false, true);
        assert!(!should_forward_ime_key("a", meta_shift, false));
    }
}
