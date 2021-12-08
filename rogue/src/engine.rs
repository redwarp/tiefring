use std::time::Instant;

use anyhow::Result;
use tiefring::{text::Font, Canvas, CanvasSettings};
use winit::{
    dpi::LogicalSize,
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use crate::{
    components::{Body, Position},
    game::Game,
    inputs::Input,
    spawner,
};

const WIDTH_IN_TILES: u32 = 25;
const HEIGHT_IN_TILES: u32 = 20;
const TILE_SIZE: f32 = 32.0;
const FONT_NAME: &str = "VT323-Regular.ttf";

pub struct Engine {}

impl Engine {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&mut self, mut game: Game) -> Result<()> {
        let event_loop = EventLoop::new();
        let mut input_helper = WinitInputHelper::new();

        let window = {
            let size = LogicalSize::new(
                WIDTH_IN_TILES as f32 * TILE_SIZE,
                HEIGHT_IN_TILES as f32 * TILE_SIZE,
            );
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

        let fonts = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("resources/fonts")
            .unwrap();
        let mut font = Font::load_font(fonts.join(FONT_NAME)).unwrap();

        let mut time = Instant::now();
        window.set_visible(true);

        let _player = spawner::player(&mut game.world, 10, 10);
        spawner::orc(&mut game.world, 3, 7);
        spawner::orc(&mut game.world, 5, 12);
        spawner::orc(&mut game.world, 14, 2);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                canvas
                    .draw(|graphics| {
                        let mut query = game.world.query::<(&Body, &Position)>();
                        query.for_each(&game.world, |(body, position)| {
                            let position = tiefring::Position::new(
                                position.x as f32 * TILE_SIZE,
                                position.y as f32 * TILE_SIZE,
                            );
                            graphics.draw_text(
                                &mut font,
                                body.char.to_string(),
                                TILE_SIZE as u32,
                                position,
                                body.color,
                            );
                        });
                    })
                    .unwrap();
            }

            if input_helper.update(&event) {
                if input_helper.quit() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                if let Some(size) = input_helper.window_resized() {
                    canvas.resize(size.width, size.height);
                }

                let now = Instant::now();
                let dt = now.duration_since(time);
                time = now;

                if game.update(dt, Input::from_input_helper(&input_helper)) {
                    *control_flow = ControlFlow::Exit;
                }
            }
        });
    }
}
