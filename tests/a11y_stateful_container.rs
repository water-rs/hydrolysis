//! A labelled container carrying semantic state of its own is a distinct
//! element: collapsing it into its only child would drop that state and
//! overwrite the child's own label and role. Regression coverage for the
//! modal-drawer panel behind water-rs/hydrolysis#148, whose decorative leaf
//! stopped emitting and left the labelled panel with a single child.
//!
//! Exercises both runtimes — the headless semantic walk (`mount`) and the
//! rendered flush (`mount_offscreen`) — since the collapse runs in each.

use hydrolysis_m3::Material3;
use waterui::ViewExt as _;
use waterui::accessibility::{AccessibilityRole, AccessibilityState};
use waterui::component::vstack;
use waterui_controls::button;
use waterui_testing::{NodeSnapshot, Role, TreeSnapshot, ui};

/// A labelled `Group` container holding exactly one child — the shape a
/// navigation drawer or disclosure panel takes once its decorative leaves
/// stop emitting nodes.
fn stateful_panel(state: AccessibilityState) -> impl waterui::View {
    vstack((button("Inbox").action(|| {}),))
        .a11y_label("Panel")
        .a11y_role(AccessibilityRole::Group)
        .a11y_state(state)
}

/// The container node must survive intact: its own role, label, and state,
/// with the child still nested beneath it.
fn assert_panel(tree: &TreeSnapshot, check_state: impl Fn(&NodeSnapshot)) {
    let panel = tree
        .nodes()
        .values()
        .find(|node| node.label() == Some("Panel"))
        .expect("the labelled panel must survive as its own node");
    assert_eq!(
        panel.role(),
        Role::GROUP,
        "the panel keeps its container role instead of collapsing into the child"
    );
    check_state(panel);
    assert_eq!(
        panel.children().len(),
        1,
        "the panel keeps its single child beneath it"
    );
    let child = tree
        .node(panel.children()[0])
        .expect("the child node is still registered");
    assert_eq!(child.role(), Role::BUTTON);
    assert_eq!(
        child.label(),
        Some("Inbox"),
        "the child keeps its own label"
    );
}

fn expanded_panel_survives(tree: &TreeSnapshot) {
    assert_panel(tree, |panel| {
        assert_eq!(panel.expanded(), Some(true));
    });
}

fn selected_panel_survives(tree: &TreeSnapshot) {
    assert_panel(tree, |panel| {
        assert!(panel.selected());
    });
}

fn toggled_panel_survives(tree: &TreeSnapshot) {
    assert_panel(tree, |panel| {
        assert_eq!(panel.checked(), Some(true));
    });
}

#[test]
fn expanded_panel_survives_on_semantic_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .mount(move || stateful_panel(AccessibilityState::new().expanded(Some(true))));
    app.settle();
    expanded_panel_survives(app.tree());
}

#[test]
fn expanded_panel_survives_on_offscreen_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .theme(Material3::defaults())
        .mount_offscreen(move || stateful_panel(AccessibilityState::new().expanded(Some(true))));
    app.settle();
    expanded_panel_survives(app.tree());
}

#[test]
fn selected_panel_survives_on_semantic_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .mount(move || stateful_panel(AccessibilityState::new().selected(true)));
    app.settle();
    selected_panel_survives(app.tree());
}

#[test]
fn selected_panel_survives_on_offscreen_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .theme(Material3::defaults())
        .mount_offscreen(move || stateful_panel(AccessibilityState::new().selected(true)));
    app.settle();
    selected_panel_survives(app.tree());
}

#[test]
fn toggled_panel_survives_on_semantic_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .mount(move || stateful_panel(AccessibilityState::new().checked(Some(true))));
    app.settle();
    toggled_panel_survives(app.tree());
}

#[test]
fn toggled_panel_survives_on_offscreen_mount() {
    let mut app = ui()
        .viewport(300, 300)
        .theme(Material3::defaults())
        .mount_offscreen(move || stateful_panel(AccessibilityState::new().checked(Some(true))));
    app.settle();
    toggled_panel_survives(app.tree());
}
