use std::time::Instant;

use anyhow::Result;
use tiefring::{text::Font, Canvas, CanvasSettings, Graphics, Rect};
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
    map::Map,
    spawner,
};

const TILE_SIZE: f32 = 32.0;
const FONT_NAME: &str = "VT323-Regular.ttf";

pub struct Engine {
    width: u32,
    height: u32,
}

impl Engine {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn run(&mut self, mut game: Game) -> Result<()> {
        let event_loop = EventLoop::new();
        let mut input_helper = WinitInputHelper::new();

        let window = {
            let size = LogicalSize::new(
                self.width as f32 * TILE_SIZE,
                self.height as f32 * TILE_SIZE,
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
                        render_game(&mut game, graphics, &mut font);
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

                window.request_redraw();
            }
        });
    }
}

fn render_game(game: &mut Game, graphics: &mut Graphics, font: &mut Font) {
    if let Some(map) = game.world.get_resource::<Map>() {
        for (j, lines) in map.lines().enumerate() {
            for (i, tile) in lines.iter().enumerate() {
                let rect = Rect::from_xywh(
                    i as f32 * TILE_SIZE,
                    j as f32 * TILE_SIZE,
                    TILE_SIZE,
                    TILE_SIZE,
                );
                graphics.draw_rect(rect, tile.color);
            }
        }
    }

    let mut query = game.world.query::<(&Body, &Position)>();
    query.for_each(&game.world, |(body, position)| {
        body.render(graphics, position, font);
    });
}

impl Body {
    fn render(&self, graphics: &mut Graphics, position: &Position, font: &mut Font) {
        let position =
            tiefring::Position::new(position.x as f32 * TILE_SIZE, position.y as f32 * TILE_SIZE);
        graphics.draw_text(
            font,
            self.char.to_string(),
            TILE_SIZE as u32,
            position,
            self.color,
        );
    }
}
