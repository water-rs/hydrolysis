//! Helpers shared by the examples. Not a Cargo target — each `examples/*.rs`
//! crate pulls this in with `mod shared;`.

use std::time::Duration;

use vello::kurbo::{BezPath, Point, Rect, RoundedRectRadii};
use waterui::Environment;
use waterui::animation::Animation;
use waterui::text::font::Font;
use waterui_backend_core::widget::{
    BadgeMetrics, Brush, ButtonMetrics, DividerMetrics, DrawContext, InputFieldMetrics,
    InteractionMotion, ListMetrics, NavigationMetrics, NavigationMotion, PickerMetrics,
    ProgressIndicatorStyle, ProgressMetrics, ProgressMotion, RadioIndicatorState,
    RadioSelectionMotion, SliderMetrics, StepperEnd, StepperMetrics, TableMetrics, TabsMetrics,
    TextCaretMotion, TextContextMenuMetrics, ToggleMetrics, WidgetInteractionState,
};
use waterui_controls::button::{ButtonSize, ButtonStyle};
use waterui_controls::toggle::ToggleStyle;
use waterui_form::picker::PickerStyle;
use waterui_graphics::color::Color;

use hydrolysis_m3::{MaterialColorScheme, MaterialTheme};

/// `hydrolysis-m3` does not implement [`hydrolysis::Style`] yet
/// (water-rs/hydrolysis-m3#64), so the examples adapt [`MaterialTheme`] with
/// this private wrapper: `install_tokens` performs the same environment
/// population `hydrolysis_m3::install_defaults` does, and every
/// `WidgetTheme` method forwards to the wrapped theme.
pub struct M3Style(MaterialTheme);

impl M3Style {
    /// Builds the dynamic light/dark Material theme bound to the color-scheme
    /// signal `hydrolysis_m3::install_defaults` installed into `env`.
    pub fn new(env: &Environment) -> Self {
        let scheme = waterui::theme::installed_color_scheme(env)
            .expect("hydrolysis_m3::install_defaults installs a color-scheme signal");
        Self(MaterialTheme::with_color_schemes(
            MaterialColorScheme::baseline_light(),
            MaterialColorScheme::baseline_dark(),
            scheme,
        ))
    }
}

impl hydrolysis::Style for M3Style {
    fn install_tokens(&self, env: &mut Environment) {
        hydrolysis_m3::install_defaults(env);
    }
}

impl hydrolysis::WidgetTheme for M3Style {
    fn interaction_motion(&self) -> InteractionMotion {
        self.0.interaction_motion()
    }

    fn progress_motion(&self) -> ProgressMotion {
        self.0.progress_motion()
    }

    fn text_caret_motion(&self) -> TextCaretMotion {
        self.0.text_caret_motion()
    }

    fn navigation_motion(&self) -> NavigationMotion {
        self.0.navigation_motion()
    }

    fn button_metrics(&self, style: ButtonStyle, size: ButtonSize) -> ButtonMetrics {
        self.0.button_metrics(style, size)
    }

    fn button_label_color(&self, _style: ButtonStyle, _disabled: bool) -> Option<Color> {
        self.0.button_label_color(_style, _disabled)
    }

    fn button_label_font(&self, _style: ButtonStyle) -> Option<Font> {
        self.0.button_label_font(_style)
    }

    fn disabled_content_alpha(&self) -> f32 {
        self.0.disabled_content_alpha()
    }

    fn draw_button_chrome(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        style: ButtonStyle,
        state: WidgetInteractionState,
    ) {
        self.0.draw_button_chrome(draw, bounds, style, state)
    }

    fn draw_button_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _style: ButtonStyle,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_button_state_layer(_draw, _bounds, _style, _state)
    }

    fn draw_interaction_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _radii: RoundedRectRadii,
        _color: vello::peniko::Color,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_interaction_state_layer(_draw, _bounds, _radii, _color, _state)
    }

    fn toggle_metrics(&self, style: ToggleStyle) -> ToggleMetrics {
        self.0.toggle_metrics(style)
    }

    fn toggle_value_animation(&self) -> Animation {
        self.0.toggle_value_animation()
    }

    fn draw_toggle_switch(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        progress: f32,
        selected: bool,
        state: WidgetInteractionState,
    ) {
        self.0
            .draw_toggle_switch(draw, bounds, progress, selected, state)
    }

    fn draw_toggle_switch_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _progress: f32,
        _selected: bool,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_toggle_switch_state_layer(_draw, _bounds, _progress, _selected, _state)
    }

    fn draw_toggle_checkbox(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        progress: f32,
        state: WidgetInteractionState,
    ) {
        self.0.draw_toggle_checkbox(draw, bounds, progress, state)
    }

    fn draw_toggle_checkbox_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _progress: f32,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_toggle_checkbox_state_layer(_draw, _bounds, _progress, _state)
    }

    fn stepper_metrics(&self) -> StepperMetrics {
        self.0.stepper_metrics()
    }

    fn draw_stepper_button(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        end: StepperEnd,
        state: WidgetInteractionState,
    ) {
        self.0.draw_stepper_button(draw, bounds, end, state)
    }

    fn draw_stepper_decrement_icon(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_stepper_decrement_icon(draw, bounds)
    }

    fn draw_stepper_increment_icon(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_stepper_increment_icon(draw, bounds)
    }

    fn draw_stepper_button_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _end: StepperEnd,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_stepper_button_state_layer(_draw, _bounds, _end, _state)
    }

    fn input_field_metrics(&self) -> InputFieldMetrics {
        self.0.input_field_metrics()
    }

    fn input_placeholder_color(&self) -> Color {
        self.0.input_placeholder_color()
    }

    fn input_selection_brush(&self) -> Brush {
        self.0.input_selection_brush()
    }

    fn input_caret_brush(&self, opacity: f32) -> Brush {
        self.0.input_caret_brush(opacity)
    }

    fn draw_input_field(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        state: WidgetInteractionState,
    ) {
        self.0.draw_input_field(draw, bounds, state)
    }

    fn draw_input_field_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _state: WidgetInteractionState,
    ) {
        self.0.draw_input_field_state_layer(_draw, _bounds, _state)
    }

    fn text_context_menu_metrics(&self) -> TextContextMenuMetrics {
        self.0.text_context_menu_metrics()
    }

    fn draw_text_context_menu_panel(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_text_context_menu_panel(draw, bounds)
    }

    fn draw_text_context_menu_separator(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_text_context_menu_separator(draw, bounds)
    }

    fn picker_metrics(&self, style: PickerStyle) -> PickerMetrics {
        self.0.picker_metrics(style)
    }

    fn radio_selection_motion(&self) -> RadioSelectionMotion {
        self.0.radio_selection_motion()
    }

    fn draw_picker_indicator(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_picker_indicator(draw, bounds)
    }

    fn draw_picker_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _state: WidgetInteractionState,
    ) {
        self.0.draw_picker_state_layer(_draw, _bounds, _state)
    }

    fn draw_picker_popup(&self, draw: &mut dyn DrawContext, popup_rect: Rect) {
        self.0.draw_picker_popup(draw, popup_rect)
    }

    fn draw_picker_popup_row_background(
        &self,
        draw: &mut dyn DrawContext,
        row_rect: Rect,
        selected: bool,
    ) {
        self.0
            .draw_picker_popup_row_background(draw, row_rect, selected)
    }

    fn draw_picker_popup_row_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _row_rect: Rect,
        _selected: bool,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_picker_popup_row_state_layer(_draw, _row_rect, _selected, _state)
    }

    fn draw_picker_separator(&self, draw: &mut dyn DrawContext, separator: Rect) {
        self.0.draw_picker_separator(draw, separator)
    }

    fn draw_radio_indicator(
        &self,
        draw: &mut dyn DrawContext,
        center: Point,
        radius: f64,
        state: RadioIndicatorState,
    ) {
        self.0.draw_radio_indicator(draw, center, radius, state)
    }

    fn draw_radio_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _center: Point,
        _radius: f64,
        _selected: bool,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_radio_state_layer(_draw, _center, _radius, _selected, _state)
    }

    fn segmented_picker_label_color(&self, _selected: bool) -> Option<Color> {
        self.0.segmented_picker_label_color(_selected)
    }

    fn draw_segmented_picker_container(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _segment_count: usize,
    ) {
        self.0
            .draw_segmented_picker_container(_draw, _bounds, _segment_count)
    }

    fn draw_segmented_picker_segment(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _selected: bool,
        _is_first: bool,
        _is_last: bool,
    ) {
        self.0
            .draw_segmented_picker_segment(_draw, _bounds, _selected, _is_first, _is_last)
    }

    fn draw_segmented_picker_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _selected: bool,
        _is_first: bool,
        _is_last: bool,
        _state: WidgetInteractionState,
    ) {
        self.0.draw_segmented_picker_state_layer(
            _draw, _bounds, _selected, _is_first, _is_last, _state,
        )
    }

    fn slider_metrics(&self) -> SliderMetrics {
        self.0.slider_metrics()
    }

    fn draw_slider_track(
        &self,
        draw: &mut dyn DrawContext,
        track_rect: Rect,
        fill_rect: Rect,
        state: WidgetInteractionState,
    ) {
        self.0.draw_slider_track(draw, track_rect, fill_rect, state)
    }

    fn draw_slider_thumb(
        &self,
        draw: &mut dyn DrawContext,
        center: Point,
        radius: f64,
        state: WidgetInteractionState,
    ) {
        self.0.draw_slider_thumb(draw, center, radius, state)
    }

    fn draw_slider_thumb_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _center: Point,
        _radius: f64,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_slider_thumb_state_layer(_draw, _center, _radius, _state)
    }

    fn progress_metrics(&self, style: ProgressIndicatorStyle) -> ProgressMetrics {
        self.0.progress_metrics(style)
    }

    fn draw_progress_linear_track(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        active_end: Option<f64>,
    ) {
        self.0.draw_progress_linear_track(draw, bounds, active_end)
    }

    fn draw_progress_linear_fill(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_progress_linear_fill(draw, bounds)
    }

    fn draw_progress_linear_indeterminate(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        elapsed: Duration,
        four_color: bool,
    ) {
        self.0
            .draw_progress_linear_indeterminate(draw, bounds, elapsed, four_color)
    }

    fn draw_progress_circular_track(
        &self,
        draw: &mut dyn DrawContext,
        center: Point,
        radius: f64,
        width: f64,
        active_turns: Option<f64>,
    ) {
        self.0
            .draw_progress_circular_track(draw, center, radius, width, active_turns)
    }

    fn draw_progress_circular_fill(&self, draw: &mut dyn DrawContext, path: &BezPath, width: f64) {
        self.0.draw_progress_circular_fill(draw, path, width)
    }

    fn draw_progress_loading(
        &self,
        draw: &mut dyn DrawContext,
        bounds: Rect,
        elapsed: Duration,
        four_color: bool,
    ) {
        self.0
            .draw_progress_loading(draw, bounds, elapsed, four_color)
    }

    fn draw_progress_circular_indeterminate(
        &self,
        draw: &mut dyn DrawContext,
        center: Point,
        radius: f64,
        width: f64,
        elapsed: Duration,
        four_color: bool,
    ) {
        self.0
            .draw_progress_circular_indeterminate(draw, center, radius, width, elapsed, four_color)
    }

    fn navigation_metrics(&self) -> NavigationMetrics {
        self.0.navigation_metrics()
    }

    fn draw_navigation_bar(&self, draw: &mut dyn DrawContext, bounds: Rect, background: &Brush) {
        self.0.draw_navigation_bar(draw, bounds, background)
    }

    fn draw_navigation_bar_separator(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_navigation_bar_separator(draw, bounds)
    }

    fn draw_navigation_back_button(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_navigation_back_button(draw, bounds)
    }

    fn tabs_metrics(&self) -> TabsMetrics {
        self.0.tabs_metrics()
    }

    fn draw_tabs_bar(&self, draw: &mut dyn DrawContext, bounds: Rect, top_edge: bool) {
        self.0.draw_tabs_bar(draw, bounds, top_edge)
    }

    fn draw_tabs_highlight(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_tabs_highlight(draw, bounds)
    }

    fn draw_tabs_button_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _selected: bool,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_tabs_button_state_layer(_draw, _bounds, _selected, _state)
    }

    fn draw_scroll_indicator(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_scroll_indicator(draw, bounds)
    }

    fn divider_metrics(&self) -> DividerMetrics {
        self.0.divider_metrics()
    }

    fn draw_divider(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_divider(draw, bounds)
    }

    fn badge_metrics(&self) -> BadgeMetrics {
        self.0.badge_metrics()
    }

    fn badge_label_color(&self) -> Color {
        self.0.badge_label_color()
    }

    fn badge_label_font(&self) -> Font {
        self.0.badge_label_font()
    }

    fn draw_badge_small(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_badge_small(draw, bounds)
    }

    fn draw_badge_large(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_badge_large(draw, bounds)
    }

    fn list_metrics(&self) -> ListMetrics {
        self.0.list_metrics()
    }

    fn draw_list_row_background(&self, draw: &mut dyn DrawContext, bounds: Rect, alternate: bool) {
        self.0.draw_list_row_background(draw, bounds, alternate)
    }

    fn draw_list_move_control(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_list_move_control(draw, bounds)
    }

    fn draw_list_move_control_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_list_move_control_state_layer(_draw, _bounds, _state)
    }

    fn draw_list_delete_control(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_list_delete_control(draw, bounds)
    }

    fn draw_list_delete_control_state_layer(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _state: WidgetInteractionState,
    ) {
        self.0
            .draw_list_delete_control_state_layer(_draw, _bounds, _state)
    }

    fn draw_list_swipe_dismiss_background(
        &self,
        _draw: &mut dyn DrawContext,
        _bounds: Rect,
        _progress: f64,
        _toward_start: bool,
    ) {
        self.0
            .draw_list_swipe_dismiss_background(_draw, _bounds, _progress, _toward_start)
    }

    fn draw_list_row_lifted(&self, _draw: &mut dyn DrawContext, _bounds: Rect, _elevation: f64) {
        self.0.draw_list_row_lifted(_draw, _bounds, _elevation)
    }

    fn draw_list_separator(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_list_separator(draw, bounds)
    }

    fn table_metrics(&self) -> TableMetrics {
        self.0.table_metrics()
    }

    fn draw_table_background(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_table_background(draw, bounds)
    }

    fn draw_table_header_background(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_table_header_background(draw, bounds)
    }

    fn draw_table_cell_border(&self, draw: &mut dyn DrawContext, bounds: Rect) {
        self.0.draw_table_cell_border(draw, bounds)
    }

    fn draw_table_column_separator(&self, draw: &mut dyn DrawContext, from: Point, to: Point) {
        self.0.draw_table_column_separator(draw, from, to)
    }
}
