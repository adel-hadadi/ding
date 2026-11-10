//! Procedural blob character painting.

use ding_client::avatar::{Accessory, Avatar, BodyShape, Eyes, Mouth};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Vec2};

use crate::gui::theme;

pub fn paint(ui: &mut egui::Ui, avatar: &Avatar, size: f32, headphones: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint_at(ui.painter(), rect.center(), size * 0.42, avatar, headphones, 1.0);
}

pub fn paint_at(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    avatar: &Avatar,
    headphones: bool,
    alpha: f32,
) {
    let body = with_alpha(theme::color_at(avatar.color), alpha);
    let shade = with_alpha(darken(body, 0.22), alpha);
    let highlight = with_alpha(lighten(body, 0.28), alpha * 0.85);
    let ink = with_alpha(theme::INK, alpha);
    let soft = with_alpha(Color32::from_black_alpha(60), alpha);

    // Soft ground shadow.
    painter.circle_filled(center + Vec2::new(0.0, radius * 0.95), radius * 0.55, soft);

    let (rx, ry) = match avatar.shape {
        BodyShape::Round => (radius, radius),
        BodyShape::Squish => (radius * 1.12, radius * 0.88),
        BodyShape::Tall => (radius * 0.86, radius * 1.14),
        BodyShape::Bean => (radius * 1.05, radius * 0.95),
    };

    // Body ellipse approximated with a filled circle + side blobs for bean/squish.
    painter.circle_filled(center + Vec2::new(0.0, ry * 0.05), (rx + ry) * 0.48, shade);
    painter.circle_filled(center, (rx + ry) * 0.46, body);
    if matches!(avatar.shape, BodyShape::Bean | BodyShape::Squish) {
        painter.circle_filled(
            center + Vec2::new(-rx * 0.35, ry * 0.15),
            radius * 0.42,
            body,
        );
        painter.circle_filled(
            center + Vec2::new(rx * 0.32, ry * 0.18),
            radius * 0.38,
            body,
        );
    }

    // Top highlight for a squishy look.
    painter.circle_filled(
        center + Vec2::new(-rx * 0.22, -ry * 0.28),
        radius * 0.22,
        highlight,
    );

    paint_accessory(painter, center, radius, avatar.accessory, ink, body, alpha);
    paint_face(painter, center, radius, avatar.eyes, avatar.mouth, ink);

    if headphones {
        paint_headphones(painter, center, radius, alpha);
    }
}

fn paint_face(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    eyes: Eyes,
    mouth: Mouth,
    ink: Color32,
) {
    let eye_y = center.y - radius * 0.08;
    let left = Pos2::new(center.x - radius * 0.28, eye_y);
    let right = Pos2::new(center.x + radius * 0.28, eye_y);
    let stroke = Stroke::new((radius * 0.08).clamp(1.5, 3.0), ink);

    match eyes {
        Eyes::Dots => {
            painter.circle_filled(left, radius * 0.08, ink);
            painter.circle_filled(right, radius * 0.08, ink);
        }
        Eyes::Happy => {
            painter.line_segment(
                [left + Vec2::new(-radius * 0.1, 0.0), left + Vec2::new(radius * 0.1, -radius * 0.08)],
                stroke,
            );
            painter.line_segment(
                [left + Vec2::new(radius * 0.1, -radius * 0.08), left + Vec2::new(radius * 0.2, 0.0)],
                stroke,
            );
            painter.line_segment(
                [right + Vec2::new(-radius * 0.1, 0.0), right + Vec2::new(radius * 0.1, -radius * 0.08)],
                stroke,
            );
            painter.line_segment(
                [right + Vec2::new(radius * 0.1, -radius * 0.08), right + Vec2::new(radius * 0.2, 0.0)],
                stroke,
            );
        }
        Eyes::Wide => {
            painter.circle_stroke(left, radius * 0.12, stroke);
            painter.circle_stroke(right, radius * 0.12, stroke);
            painter.circle_filled(left, radius * 0.05, ink);
            painter.circle_filled(right, radius * 0.05, ink);
        }
        Eyes::Sleepy => {
            painter.line_segment(
                [left + Vec2::new(-radius * 0.12, 0.0), left + Vec2::new(radius * 0.12, 0.0)],
                stroke,
            );
            painter.line_segment(
                [right + Vec2::new(-radius * 0.12, 0.0), right + Vec2::new(radius * 0.12, 0.0)],
                stroke,
            );
        }
        Eyes::Wink => {
            painter.circle_filled(left, radius * 0.08, ink);
            painter.line_segment(
                [right + Vec2::new(-radius * 0.12, 0.0), right + Vec2::new(radius * 0.12, 0.0)],
                stroke,
            );
        }
    }

    let mouth_c = Pos2::new(center.x, center.y + radius * 0.28);
    match mouth {
        Mouth::Smile => {
            let points = arc_points(mouth_c, radius * 0.22, 20.0, 160.0, 10);
            painter.add(Shape::line(points, stroke));
        }
        Mouth::Grin => {
            let points = arc_points(mouth_c, radius * 0.28, 15.0, 165.0, 12);
            painter.add(Shape::line(points, Stroke::new(stroke.width + 0.5, ink)));
        }
        Mouth::Oh => {
            painter.circle_stroke(mouth_c, radius * 0.1, stroke);
        }
        Mouth::Flat => {
            painter.line_segment(
                [
                    mouth_c + Vec2::new(-radius * 0.16, 0.0),
                    mouth_c + Vec2::new(radius * 0.16, 0.0),
                ],
                stroke,
            );
        }
        Mouth::Smirk => {
            let points = arc_points(
                mouth_c + Vec2::new(radius * 0.06, 0.0),
                radius * 0.18,
                10.0,
                120.0,
                8,
            );
            painter.add(Shape::line(points, stroke));
        }
    }
}

fn paint_accessory(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    accessory: Accessory,
    ink: Color32,
    body: Color32,
    alpha: f32,
) {
    match accessory {
        Accessory::None => {}
        Accessory::Glasses => {
            let y = center.y - radius * 0.08;
            let stroke = Stroke::new((radius * 0.07).clamp(1.4, 2.6), ink);
            painter.circle_stroke(Pos2::new(center.x - radius * 0.28, y), radius * 0.18, stroke);
            painter.circle_stroke(Pos2::new(center.x + radius * 0.28, y), radius * 0.18, stroke);
            painter.line_segment(
                [
                    Pos2::new(center.x - radius * 0.1, y),
                    Pos2::new(center.x + radius * 0.1, y),
                ],
                stroke,
            );
        }
        Accessory::Cap => {
            let brim = with_alpha(Color32::from_rgb(59, 130, 246), alpha);
            let top = with_alpha(Color32::from_rgb(37, 99, 235), alpha);
            painter.circle_filled(center + Vec2::new(0.0, -radius * 0.72), radius * 0.42, top);
            painter.rect_filled(
                egui::Rect::from_center_size(
                    center + Vec2::new(radius * 0.15, -radius * 0.48),
                    Vec2::new(radius * 1.15, radius * 0.18),
                ),
                radius * 0.1,
                brim,
            );
        }
        Accessory::Sprout => {
            let leaf = with_alpha(Color32::from_rgb(74, 222, 128), alpha);
            let stem = Stroke::new((radius * 0.07).clamp(1.4, 2.4), ink);
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -radius * 0.55),
                    center + Vec2::new(0.0, -radius * 0.95),
                ],
                stem,
            );
            painter.circle_filled(center + Vec2::new(-radius * 0.16, -radius * 0.95), radius * 0.16, leaf);
            painter.circle_filled(center + Vec2::new(radius * 0.18, -radius * 1.0), radius * 0.14, leaf);
        }
        Accessory::Bow => {
            let bow = with_alpha(Color32::from_rgb(244, 114, 182), alpha);
            painter.circle_filled(center + Vec2::new(-radius * 0.22, -radius * 0.7), radius * 0.16, bow);
            painter.circle_filled(center + Vec2::new(radius * 0.22, -radius * 0.7), radius * 0.16, bow);
            painter.circle_filled(center + Vec2::new(0.0, -radius * 0.68), radius * 0.1, darken(bow, 0.15));
            let _ = body;
        }
    }
}

fn paint_headphones(painter: &egui::Painter, center: Pos2, radius: f32, alpha: f32) {
    let band = with_alpha(Color32::from_rgb(60, 56, 80), alpha);
    let cup = with_alpha(Color32::from_rgb(139, 92, 246), alpha);
    let stroke = Stroke::new((radius * 0.1).clamp(2.0, 3.5), band);
    let points = arc_points(center + Vec2::new(0.0, -radius * 0.05), radius * 1.05, 200.0, 340.0, 18);
    painter.add(Shape::line(points, stroke));
    painter.circle_filled(center + Vec2::new(-radius * 0.95, 0.0), radius * 0.22, cup);
    painter.circle_filled(center + Vec2::new(radius * 0.95, 0.0), radius * 0.22, cup);
}

fn arc_points(center: Pos2, radius: f32, start_deg: f32, end_deg: f32, steps: usize) -> Vec<Pos2> {
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let deg = start_deg + (end_deg - start_deg) * t;
        let rad = deg.to_radians();
        points.push(Pos2::new(
            center.x + rad.cos() * radius,
            center.y + rad.sin() * radius,
        ));
    }
    points
}

fn darken(color: Color32, amount: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (color.r() as f32 * (1.0 - amount)) as u8,
        (color.g() as f32 * (1.0 - amount)) as u8,
        (color.b() as f32 * (1.0 - amount)) as u8,
        color.a(),
    )
}

fn lighten(color: Color32, amount: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (color.r() as f32 + (255.0 - color.r() as f32) * amount) as u8,
        (color.g() as f32 + (255.0 - color.g() as f32) * amount) as u8,
        (color.b() as f32 + (255.0 - color.b() as f32) * amount) as u8,
        color.a(),
    )
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (color.a() as f32 * alpha) as u8)
}

/// Tiny logo bell-blob used in the sidebar.
pub fn paint_logo_bell(ui: &mut egui::Ui, size: f32, awake: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let time = ui.input(|input| input.time as f32);
    let wobble = if awake {
        (time * 10.0).sin() * 3.0
    } else {
        (time * 1.6).sin() * 1.2
    };
    let center = rect.center() + Vec2::new(wobble, 0.0);
    let avatar = Avatar {
        shape: BodyShape::Squish,
        color: 0,
        eyes: Eyes::Happy,
        mouth: Mouth::Smile,
        accessory: Accessory::None,
    };
    paint_at(ui.painter(), center, size * 0.38, &avatar, false, 1.0);
}
