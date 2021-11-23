use tefrung::{Canvas, Color, Position};
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
    println!("Hello, world!");

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello Tēfrung")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_visible(false)
            .build(&event_loop)
            .unwrap()
    };

    let mut canvas = {
        let window_size = window.inner_size();
        pollster::block_on(Canvas::new(&window, window_size.width, window_size.height))
    }
    .unwrap();

    let sprite = canvas.load_sprite("sample/sprites/p1_jump.png").unwrap();

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
                        [50, 50, 150, 150],
                        Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.5,
                        },
                    );
                    graphics.draw_rect(
                        [-75, -75, -50, -50],
                        Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 0.75,
                        },
                    );
                    for index in 0..100 {
                        graphics.draw_sprite(
                            Position {
                                left: index as f32 * 10.0,
                                top: index as f32 * 10.0,
                            },
                            &sprite,
                        );
                    }
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
