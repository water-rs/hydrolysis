use hydrolysis::{OffscreenSurface, SurfaceProvider};
use cherenkov::kurbo::{Affine, Rect};
use cherenkov::{Picture, WorkingColor, Draw as _};

#[test]
fn raw_picture_renders() {
    let surface = OffscreenSurface::new_for_tests(64, 64);
    let inner = Picture::record(|r| { r.fill(Rect::new(0.0, 0.0, 32.0, 32.0), WorkingColor::new([1.0, 0.0, 0.0, 1.0])); });
    let outer = Picture::record(|r| { r.picture(&inner, Affine::IDENTITY); });
    let s = SurfaceProvider::surface(&surface);
    s.clear_color(WorkingColor::WHITE);
    s.update(|tx| { tx[s.root()].content(outer); });
    surface.gpu().engine().render(cherenkov::FrameTime::at(std::time::Instant::now())).unwrap();
    let px = surface.readback_rgba8();
    let mut hist = std::collections::BTreeMap::new();
    for p in px.chunks(4) { *hist.entry(p.to_vec()).or_insert(0usize) += 1; }
    eprintln!("{hist:?}");
    // direct
    let direct = Picture::record(|r| { r.fill(Rect::new(0.0, 0.0, 32.0, 32.0), WorkingColor::new([0.0, 1.0, 0.0, 1.0])); });
    s.update(|tx| { tx[s.root()].content(direct); });
    surface.gpu().engine().render(cherenkov::FrameTime::at(std::time::Instant::now())).unwrap();
    let px = surface.readback_rgba8();
    let mut hist = std::collections::BTreeMap::new();
    for p in px.chunks(4) { *hist.entry(p.to_vec()).or_insert(0usize) += 1; }
    eprintln!("{hist:?}");
}

#[test]
fn view_renders() {
    use hydrolysis::HydrolysisRenderer;
    use waterui::ViewExt as _;
    let surface = OffscreenSurface::new_for_tests(64, 64);
    let theme: std::rc::Rc<dyn waterui_backend_core::WidgetTheme> = std::rc::Rc::new(hydrolysis_m3::Material3::defaults());
    let mut renderer = HydrolysisRenderer::new(std::rc::Rc::clone(surface.gpu().engine()), theme);
    let env = waterui::Environment::new();
    let view = waterui::layout::frame::Frame::new(waterui::component::Spacer::new(1.0)).width(32.0).height(32.0).background(waterui::Color::srgb(255, 0, 0));
    renderer.reset_scene();
    renderer.begin_rebuild_frame();
    renderer.capture_window_tree(waterui::AnyView::new(view), &env, cherenkov::kurbo::Rect::new(0.0,0.0,64.0,64.0), Affine::IDENTITY, Affine::IDENTITY);
    renderer.finish_rebuild_frame();
    let s = SurfaceProvider::surface(&surface);
    renderer.present_frame(s, WorkingColor::WHITE, std::time::Instant::now()).unwrap();
    let px = surface.readback_rgba8();
    let mut hist = std::collections::BTreeMap::new();
    for p in px.chunks(4) { *hist.entry(p.to_vec()).or_insert(0usize) += 1; }
    eprintln!("VIEW {hist:?}");
}
