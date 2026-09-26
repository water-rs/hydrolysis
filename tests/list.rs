//! List widget regressions.

//! <https://github.com/water-rs/hydrolysis/issues/168>: `apply_scroll_request`
//! asserted `index < row_count`, but the row count comes from signal-driven
//! contents — a `List` materializing mid-flush with a pending target above its
//! current row count panicked inside `list_accessibility`. The watergram
//! dogfood hits it on every chat open: the message list clears to 0 rows while
//! the previous contents' target is still pending. A scroll request names a
//! row that may not exist yet: it stays pending until the contents contain the
//! index, and a newer generation supersedes it.

use std::cell::Cell;
use std::rc::Rc;

use hydrolysis_m3::Material3;
use nami::collection::List as ReactiveList;
use waterui::component::list::{List, ListDelete, ListItem};
use waterui::component::{hstack, spacer, text};
use waterui::id::SelfId;
use waterui::layout::scroll::ScrollController;
use waterui::{Binding, View, ViewExt};
use waterui_core::dynamic::watch;
use waterui_testing::{ui, Role};

/// Material's one-line list row: a scroll request landing on row `N` reports a
/// `scroll_y` of `N * ROW_HEIGHT` on the rendered runtime.
const ROW_HEIGHT: f64 = 56.0;
/// The pending target from the issue — a row the mounted contents do not have.
const SCROLL_TARGET: usize = 8;
/// The contents grow past the target after mount, so the pending request
/// becomes actionable.
const GROWN_ROW_COUNT: usize = 20;

/// The lazy identity-keyed list from the issue, with its scroll controller
/// attached and an accessibility label to query it by.
fn pending_scroll_list(
    items: ReactiveList<SelfId<usize>>,
    controller: ScrollController<usize>,
) -> impl View {
    List::for_each(items, |item| ListItem::new(text(format!("row {}", *item))))
        .scroll_controller(&controller)
        .a11y_label("messages")
}

/// The semantic runtime runs `list_accessibility` with no render context —
/// its scroll domain is row units, so a landed request reports the target
/// index as `scroll_y`.
#[test]
fn pending_scroll_target_above_row_count_waits_for_contents_semantic() {
    let items = ReactiveList::<SelfId<usize>>::new();
    let controller = ScrollController::new(0);
    // The request lands while the contents are still empty: the dogfood's
    // cleared message list carried a pending target in exactly this shape.
    controller.scroll_to(SCROLL_TARGET);
    let mut app = ui().mount({
        let items = items.clone();
        let controller = controller.clone();
        move || pending_scroll_list(items.clone(), controller.clone())
    });
    // Growing the contents past the target makes the pending request
    // actionable; it must not have been dropped while unreachable.
    let _ = items.replace((0..GROWN_ROW_COUNT).map(SelfId::new).collect());
    app.settle();
    let scroll_y = app
        .query()
        .role(Role::LIST)
        .label("messages")
        .single()
        .node()
        .scroll_y()
        .expect("the list reports a scroll offset");
    assert!(
        (scroll_y - SCROLL_TARGET as f64).abs() < 0.5,
        "pending scroll should land on row {SCROLL_TARGET} once the contents reach it: scroll_y={scroll_y}"
    );
}

/// The rendered runtime takes the same `apply_scroll_request` path through
/// `render_list`; its scroll domain is points, and the virtualized window
/// shows which rows are realized.
#[test]
fn pending_scroll_target_above_row_count_waits_for_contents_offscreen() {
    let items = ReactiveList::<SelfId<usize>>::new();
    let controller = ScrollController::new(0);
    controller.scroll_to(SCROLL_TARGET);
    let mut app = ui()
        .viewport(320, 320)
        .theme(Material3::defaults())
        .mount_offscreen({
            let items = items.clone();
            let controller = controller.clone();
            move || pending_scroll_list(items.clone(), controller.clone())
        });
    let _ = items.replace((0..GROWN_ROW_COUNT).map(SelfId::new).collect());
    app.settle();
    let scroll_y = app
        .query()
        .role(Role::LIST)
        .label("messages")
        .single()
        .node()
        .scroll_y()
        .expect("the list reports a scroll offset");
    let expected = SCROLL_TARGET as f64 * ROW_HEIGHT;
    assert!(
        (scroll_y - expected).abs() < 1.0,
        "pending scroll should land on row {SCROLL_TARGET}: expected scroll_y≈{expected}, got {scroll_y}"
    );
    // The landed scroll moved the virtualized window: the target row is
    // realized, row 0 scrolled out and was evicted.
    app.query().label("row 8").assert_exists();
    app.query().label("row 0").assert_not_exists();
}

/// <https://github.com/water-rs/hydrolysis/issues/111>: a tap on `List` row
/// content never fires once the row's content re-renders while the row's swipe
/// gesture stays armed. The retained swipe re-registers with the hit-test order
/// minted on its birth frame while the rebuilt tap mints a fresh order — and
/// the order counter resets every rebuild, so the stale order can permanently
/// outrank the tap: `top_group_id_at` then picks the swipe's gesture group and
/// the tap recognizer never activates.
#[test]
fn row_content_tap_survives_retained_swipe_order_offscreen() {
    let highlight = Binding::bool(false);
    let taps = Rc::new(Cell::new(0));
    let mut app = ui()
        .viewport(360, 240)
        .theme(Material3::defaults())
        .mount_offscreen({
            let taps = Rc::clone(&taps);
            let highlight = highlight.clone();
            move || {
                let highlight = highlight.clone();
                let taps = Rc::clone(&taps);
                let items =
                    ReactiveList::from((1..=3).map(SelfId::new).collect::<Vec<SelfId<i32>>>());
                List::for_each(items, move |item| {
                    let row = *item;
                    let taps = Rc::clone(&taps);
                    ListItem::new(watch(highlight.clone(), move |on| {
                        let label = if on { "high" } else { "low" };
                        let taps = Rc::clone(&taps);
                        hstack((text(format!("row {row} {label}")), spacer()))
                            .on_tap(move || taps.set(row))
                    }))
                })
                .on_delete(|_: ListDelete| {})
            }
        });

    app.query().label_contains("row 2").tap_at(0.5, 0.5);
    assert_eq!(taps.get(), 2, "baseline: row content on_tap must fire");

    // Rebuilding the row contents re-mints their tap targets while the rows'
    // swipe gestures stay retained; the taps must still outrank them.
    highlight.set(true);
    app.settle();

    taps.set(0);
    app.query().label_contains("row 2").tap_at(0.5, 0.5);
    assert_eq!(
        taps.get(),
        2,
        "row content on_tap must still fire after the contents rebuild"
    );
}
