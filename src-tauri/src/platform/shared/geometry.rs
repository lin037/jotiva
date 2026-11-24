use crate::{desktop::app, host::Rect};
use tauri::{LogicalPosition, Monitor, PhysicalPosition, WebviewWindow};

fn area(m: &Monitor) -> Rect {
    let work = m.work_area();
    Rect {
        x: work.position.x as f64,
        y: work.position.y as f64,
        width: work.size.width as f64 / m.scale_factor(),
        height: work.size.height as f64 / m.scale_factor(),
    }
}
fn monitor(at: LogicalPosition<f64>) -> Option<Monitor> {
    app().available_monitors().ok()?.into_iter().min_by(|a, b| {
        let distance = |m: &Monitor| {
            let r = area(m);
            (at.x - at.x.clamp(r.x, r.x + r.width)).hypot(at.y - at.y.clamp(r.y, r.y + r.height))
        };
        distance(a).total_cmp(&distance(b))
    })
}
pub fn screen_at(at: LogicalPosition<f64>) -> Rect {
    monitor(at).as_ref().map(area).unwrap_or_default()
}
pub fn rect_at_pixel(x: f64, y: f64) -> Rect {
    app()
        .monitor_from_point(x, y)
        .ok()
        .flatten()
        .or_else(|| app().primary_monitor().ok().flatten())
        .as_ref()
        .map(area)
        .unwrap_or_default()
}
pub fn visible_screen() -> Rect {
    let p = app().cursor_position().unwrap_or_default();
    rect_at_pixel(p.x, p.y)
}
fn logical(x: f64, y: f64) -> LogicalPosition<f64> {
    let Some(m) = app().monitor_from_point(x, y).ok().flatten() else {
        return LogicalPosition::new(x, y);
    };
    let r = area(&m);
    LogicalPosition::new(
        r.x + (x - r.x) / m.scale_factor(),
        r.y + (y - r.y) / m.scale_factor(),
    )
}
pub fn physical(at: LogicalPosition<f64>) -> PhysicalPosition<i32> {
    let Some(m) = monitor(at) else {
        return at.to_physical(1.0);
    };
    let r = area(&m);
    PhysicalPosition::new(
        (r.x + (at.x - r.x) * m.scale_factor()).round() as i32,
        (r.y + (at.y - r.y) * m.scale_factor()).round() as i32,
    )
}
pub fn pointer() -> LogicalPosition<f64> {
    let p = app().cursor_position().unwrap_or_default();
    logical(p.x, p.y)
}
pub fn window_origin(window: &WebviewWindow) -> tauri::Result<LogicalPosition<f64>> {
    let p = window.outer_position()?;
    Ok(logical(p.x.into(), p.y.into()))
}
