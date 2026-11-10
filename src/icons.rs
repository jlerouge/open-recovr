//! Simple line icons drawn with shapes, so they never depend on a font.

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Shield,
    Photo,
    Video,
    Music,
    Document,
    Archive,
    Folder,
    Stop,
    Play,
    Pointer,
    Warning,
    Check,
    Globe,
    Sun,
    Moon,
    Palette,
}

/// Draws `icon` centred on `c`, about `s` points wide.
pub fn draw(p: &Painter, icon: Icon, c: Pos2, s: f32, color: Color32) {
    let k = s / 20.0; // icons are designed on a 20×20 grid
    let st = Stroke::new((1.8 * k).max(1.2), color);
    let at = |x: f32, y: f32| pos2(c.x + (x - 10.0) * k, c.y + (y - 10.0) * k);
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    let r = |v: f32| CornerRadius::same((v * k).round().clamp(0.0, 255.0) as u8);
    match icon {
        Icon::Search => {
            p.circle_stroke(at(8.5, 8.5), 5.8 * k, Stroke::new(2.2 * k, color));
            p.line_segment([at(12.8, 12.8), at(17.5, 17.5)], Stroke::new(2.6 * k, color));
        }
        Icon::Shield => {
            let pts = vec![at(10.0, 2.0), at(16.5, 4.5), at(16.0, 10.5), at(10.0, 18.0), at(4.0, 10.5), at(3.5, 4.5)];
            p.add(Shape::closed_line(pts, st));
            p.add(Shape::line(vec![at(7.0, 10.0), at(9.3, 12.3), at(13.3, 7.8)], st));
        }
        Icon::Photo => {
            p.rect_stroke(rect(2.5, 4.0, 17.5, 16.0), r(2.5), st, StrokeKind::Middle);
            p.circle_filled(at(7.0, 8.0), 1.5 * k, color);
            p.add(Shape::line(vec![at(3.5, 14.5), at(8.0, 10.5), at(11.0, 13.0), at(13.5, 10.8), at(16.5, 13.8)], st));
        }
        Icon::Video => {
            p.rect_stroke(rect(2.5, 4.0, 17.5, 16.0), r(2.5), st, StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at(8.2, 7.0), at(13.6, 10.0), at(8.2, 13.0)], color, Stroke::NONE));
        }
        Icon::Music => {
            p.line_segment([at(8.0, 14.5), at(8.0, 4.0)], st);
            p.line_segment([at(15.5, 12.5), at(15.5, 2.5)], st);
            p.add(Shape::convex_polygon(vec![at(8.0, 4.0), at(15.5, 2.5), at(15.5, 5.0), at(8.0, 6.5)], color, Stroke::NONE));
            p.circle_filled(at(6.0, 14.8), 2.4 * k, color);
            p.circle_filled(at(13.5, 12.8), 2.4 * k, color);
        }
        Icon::Document => {
            p.add(Shape::closed_line(vec![at(4.5, 2.5), at(11.5, 2.5), at(15.5, 6.5), at(15.5, 17.5), at(4.5, 17.5)], st));
            p.add(Shape::line(vec![at(11.5, 2.5), at(11.5, 6.5), at(15.5, 6.5)], st));
            p.line_segment([at(7.0, 10.0), at(13.0, 10.0)], st);
            p.line_segment([at(7.0, 13.5), at(13.0, 13.5)], st);
        }
        Icon::Archive => {
            p.rect_stroke(rect(2.5, 3.5, 17.5, 7.5), r(1.0), st, StrokeKind::Middle);
            p.add(Shape::line(vec![at(3.8, 7.5), at(3.8, 16.5), at(16.2, 16.5), at(16.2, 7.5)], st));
            p.line_segment([at(8.0, 11.0), at(12.0, 11.0)], st);
        }
        Icon::Folder => {
            p.add(Shape::closed_line(vec![at(2.5, 4.5), at(8.0, 4.5), at(9.8, 6.5), at(17.5, 6.5), at(17.5, 15.5), at(2.5, 15.5)], st));
        }
        Icon::Stop => {
            p.rect_filled(rect(5.0, 5.0, 15.0, 15.0), r(2.0), color);
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![at(6.0, 4.0), at(16.0, 10.0), at(6.0, 16.0)], color, Stroke::NONE));
        }
        Icon::Pointer => {
            let pts = vec![at(5.0, 2.5), at(15.5, 12.0), at(10.5, 12.5), at(13.0, 17.5), at(11.0, 18.3), at(8.5, 13.4), at(5.0, 16.5)];
            p.add(Shape::closed_line(pts, st));
        }
        Icon::Warning => {
            p.add(Shape::closed_line(vec![at(10.0, 2.5), at(18.0, 17.0), at(2.0, 17.0)], st));
            p.line_segment([at(10.0, 7.5), at(10.0, 12.0)], st);
            p.circle_filled(at(10.0, 14.5), 1.1 * k, color);
        }
        Icon::Check => {
            p.add(Shape::line(vec![at(4.0, 10.5), at(8.3, 14.5), at(16.0, 5.5)], Stroke::new(2.4 * k, color)));
        }
        Icon::Globe => {
            p.circle_stroke(at(10.0, 10.0), 7.5 * k, st);
            p.line_segment([at(2.5, 10.0), at(17.5, 10.0)], st);
            p.line_segment([at(3.8, 6.2), at(16.2, 6.2)], st);
            p.line_segment([at(3.8, 13.8), at(16.2, 13.8)], st);
            // the meridian, as a narrow ellipse
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    at(10.0 + 3.4 * a.cos(), 10.0 + 7.5 * a.sin())
                })
                .collect();
            p.add(Shape::line(pts, st));
        }
        Icon::Sun => {
            p.circle_stroke(at(10.0, 10.0), 3.6 * k, st);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                p.line_segment([at(10.0 + 6.3 * c, 10.0 + 6.3 * s), at(10.0 + 8.6 * c, 10.0 + 8.6 * s)], st);
            }
        }
        Icon::Moon => {
            // a crescent: the part of a disc that a second, offset disc doesn't cover
            let (o, big, cut, small): ((f32, f32), f32, (f32, f32), f32) = ((10.0, 10.5), 7.0, (13.8, 6.4), 5.6);
            let ring = |(cx, cy): (f32, f32), r: f32| -> Vec<(f32, f32)> {
                let start = (cut.1 - o.1).atan2(cut.0 - o.0);
                (0..=64).map(|i| start + i as f32 / 64.0 * std::f32::consts::TAU).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect()
            };
            let inside = |(x, y): (f32, f32), (cx, cy): (f32, f32), r: f32| (x - cx).powi(2) + (y - cy).powi(2) < r * r;
            let mut pts: Vec<Pos2> = ring(o, big).into_iter().filter(|&q| !inside(q, cut, small)).map(|(x, y)| at(x, y)).collect();
            let inner: Vec<Pos2> = ring(cut, small).into_iter().filter(|&q| inside(q, o, big)).map(|(x, y)| at(x, y)).collect();
            pts.extend(inner.into_iter().rev());
            p.add(Shape::closed_line(pts, st));
        }
        Icon::Palette => {
            p.circle_stroke(at(10.0, 10.0), 7.5 * k, st);
            for (x, y) in [(6.6, 8.2), (10.0, 5.8), (13.4, 8.2), (12.6, 12.6)] {
                p.circle_filled(at(x, y), 1.4 * k, color);
            }
        }
    }
}
