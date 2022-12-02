use std::f32::consts::PI;

use fps_counter::FPSCounter;
use tiefring::{Canvas, CanvasSettings, Color, Position};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

fn main() {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello Tīefring")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_visible(false)
            .build(&event_loop)
            .unwrap()
    };

    let mut canvas = {
        let window_size = window.inner_size();
        pollster::block_on(Canvas::new(
            &window,
            window_size.width,
            window_size.height,
            CanvasSettings {
                background_color: Color {
                    r: 0.3,
                    g: 0.2,
                    b: 0.4,
                    a: 1.0,
                },
                scale: 1.0,
            },
        ))
    }
    .unwrap();

    let sprites = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("sample/sprites")
        .unwrap();

    let resources = canvas.resources();

    let alien_1 = resources.load_sprite(sprites.join("p1_jump.png")).unwrap();
    let alien_2 = resources.load_sprite(sprites.join("p2_front.png")).unwrap();
    let alien_3 = resources.load_sprite(sprites.join("p3_stand.png")).unwrap();
    let rocket = resources.load_svg(sprites.join("rocket.svg")).unwrap();

    let tile_set = resources
        .load_tileset(sprites.join("basictiles.png"), (16, 16))
        .unwrap();

    let fonts = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("resources/fonts")
        .unwrap();

    let mut roboto_regular = resources
        .load_font(fonts.join("Roboto-Regular.ttf"))
        .unwrap();
    let mut vt323_regular = resources
        .load_font(fonts.join("VT323-Regular.ttf"))
        .unwrap();

    window.set_visible(true);
    let mut translation = Position::new(0.0, 0.0);
    let mut angle = PI / 4.0;
    let mut fps_counter = FPSCounter::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        if let Event::MainEventsCleared = event {
            canvas
                .draw(|graphics| {
                    graphics.with_translation(translation, |graphics| {
                        graphics.draw_rect(
                            [0, 0, 100, 100],
                            Color {
                                r: 1.0,
                                g: 1.0,
                                b: 0.0,
                                a: 1.0,
                            },
                        );
                        graphics.draw_rect(
                            [50, 50, 150, 150],
                            Color {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.5,
                            },
                        );
                        graphics.draw_sprite(&alien_1, Position { x: 10.0, y: 100.0 });
                        graphics.draw_sprite(&alien_2, Position { x: 77.0, y: 100.0 });
                        graphics.draw_sprite(&alien_3, Position { x: 144.0, y: 100.0 });
                        graphics.draw_rect(
                            [0, 160, 240, 200],
                            Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            },
                        );

                        graphics
                            .draw_sprite_in_rect(&alien_1, [211, 100, 134, 188])
                            .rotate(angle)
                            .translate(100.0, 0.0);
                        graphics.draw_sprite(&alien_1, Position { x: 150.0, y: 200.0 });

                        let (x, y) = tile_set.tile_count();
                        for i in 0..x {
                            for j in 0..y {
                                let sprite = tile_set.sprite(i, j);
                                graphics.draw_sprite(
                                    sprite,
                                    Position {
                                        x: i as f32 * 16.0 + 300.0,
                                        y: j as f32 * 16.0,
                                    },
                                );
                            }
                        }

                        graphics.draw_sprite(&alien_1, Position { x: 350.0, y: 0.0 });

                        graphics.draw_text(
                            &mut roboto_regular,
                            "Wehp this is some text it seems!\n(With some line break, too.)",
                            32,
                            Position::new(0.0, 0.0),
                            Color {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            },
                        );
                        graphics.draw_text(
                            &mut roboto_regular,
                            "And even more text!",
                            32,
                            Position::new(50.0, 200.0),
                            Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.5,
                            },
                        );
                        graphics.draw_text(
                            &mut vt323_regular,
                            "Let's see how monospace\nfonts behave.\nPretty good it seems!ç",
                            20,
                            Position::new(20.0, 300.0),
                            Color {
                                r: 2.0,
                                g: 2.0,
                                b: 2.0,
                                a: 0.75,
                            },
                        );
                        graphics
                            .draw_sprite(&rocket, Position::new(300.0, 150.0))
                            .rotate(-0.25);

                        graphics.draw_text(
                            &mut vt323_regular,
                            format!("FPS: {}", fps_counter.tick()),
                            20,
                            Position::new(10.0, 400.0),
                            Color::rgb(1.0, 1.0, 1.0),
                        );
                    });
                })
                .unwrap();
        }

        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            } else if input.key_pressed(VirtualKeyCode::NumpadAdd) {
                increase_scale(&mut canvas);
            } else if input.key_pressed(VirtualKeyCode::NumpadSubtract) {
                decrease_scale(&mut canvas);
            } else if input.key_pressed(VirtualKeyCode::Up) {
                translate(0.0, -10.0, &mut translation);
            } else if input.key_pressed(VirtualKeyCode::Down) {
                translate(0.0, 10.0, &mut translation);
            } else if input.key_pressed(VirtualKeyCode::Left) {
                translate(-10.0, 0.0, &mut translation);
            } else if input.key_pressed(VirtualKeyCode::Right) {
                translate(10.0, 0.0, &mut translation);
            } else if input.key_pressed(VirtualKeyCode::Q) {
                angle -= PI / 42.0
            } else if input.key_pressed(VirtualKeyCode::E) {
                angle += PI / 42.0
            }

            if input.key_pressed(VirtualKeyCode::P) {
                pollster::block_on(canvas.screenshot("screenshot.png")).unwrap();
            }

            if let Some(size) = input.window_resized() {
                canvas.set_size(size.width, size.height);
            }

            window.request_redraw();
        }
    });
}

fn increase_scale(canvas: &mut Canvas) {
    let scale = (canvas.scale() + 0.1).min(2.0);
    canvas.set_scale(scale);
}

fn decrease_scale(canvas: &mut Canvas) {
    let scale = (canvas.scale() - 0.1).max(1.0);
    canvas.set_scale(scale);
}

fn translate(dx: f32, dy: f32, translation: &mut Position) {
    translation.x = (translation.x + dx).max(0.0);
    translation.y = (translation.y + dy).max(0.0);
}
