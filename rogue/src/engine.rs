use std::time::Instant;

use anyhow::Result;
use tiefring::{Canvas, CanvasSettings};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use crate::game::Game;

pub struct Engine {}

impl Engine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&mut self, mut game: Game) -> Result<()> {
        let event_loop = EventLoop::new();
        let mut input = WinitInputHelper::new();

        let window = {
            let size = LogicalSize::new(600.0, 400.0);
            WindowBuilder::new()
                .with_title("Rogue")
                .with_inner_size(size)
                .with_resizable(false)
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
                CanvasSettings::default(),
            ))
        }?;

        let mut time = Instant::now();
        window.set_visible(true);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                canvas.draw(|_graphics| {}).unwrap();
            }

            if input.update(&event) {
                if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                if let Some(size) = input.window_resized() {
                    canvas.resize(size.width, size.height);
                }

                let now = Instant::now();
                let dt = now.duration_since(time);
                time = now;

                game.update(dt);
            }
        });
    }
}
