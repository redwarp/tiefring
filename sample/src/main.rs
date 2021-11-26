use tiefring::{
    sprite::{Sprite, TileSet},
    Canvas, CanvasSettings, Color, Position,
};
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
                ..Default::default()
            },
        ))
    }
    .unwrap();

    let sprites = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("sample/sprites")
        .unwrap();

    let alien_1 = Sprite::load_image(&mut canvas, sprites.join("p1_jump.png")).unwrap();
    let alien_2 = Sprite::load_image(&mut canvas, sprites.join("p2_front.png")).unwrap();
    let alien_3 = Sprite::load_image(&mut canvas, sprites.join("p3_stand.png")).unwrap();
    let tile_set =
        TileSet::load_image(&mut canvas, sprites.join("basictiles.png"), (16, 16)).unwrap();

    window.set_visible(true);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        if let Event::RedrawRequested(_) = event {
            canvas
                .draw(|graphics| {
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
                        [50, 50, 100, 100],
                        Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.5,
                        },
                    );
                    graphics.draw_sprite(
                        &alien_1,
                        Position {
                            left: 10.0,
                            top: 100.0,
                        },
                    );
                    graphics.draw_sprite(
                        &alien_2,
                        Position {
                            left: 77.0,
                            top: 100.0,
                        },
                    );
                    graphics.draw_sprite(
                        &alien_3,
                        Position {
                            left: 144.0,
                            top: 100.0,
                        },
                    );
                    graphics.draw_rect(
                        [0, 160, 240, 120],
                        Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        },
                    );
                    graphics.draw_sprite_in_rect(&alien_1, [211, 100, 134, 188]);
                    graphics.draw_sprite(
                        &alien_1,
                        Position {
                            left: 150.0,
                            top: 200.0,
                        },
                    );

                    let (x, y) = tile_set.tile_count();
                    for i in 0..x {
                        for j in 0..y {
                            let sprite = tile_set.sprite(i, j);
                            graphics.draw_sprite(
                                &sprite,
                                Position {
                                    left: i as f32 * 16.0 + 300.0,
                                    top: j as f32 * 16.0,
                                },
                            )
                        }
                    }

                    graphics.draw_sprite(
                        &alien_1,
                        Position {
                            left: 350.0,
                            top: 0.0,
                        },
                    );
                })
                .unwrap();
        }

        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            if let Some(size) = input.window_resized() {
                canvas.resize(size.width, size.height);
            }

            window.request_redraw();
        }
    });
}
