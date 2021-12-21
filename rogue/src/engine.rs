use anyhow::Result;
use bevy_ecs::prelude::Mut;
use tiefring::{text::Font, Canvas, CanvasSettings, Color, Graphics, Rect, Size};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use crate::{
    components::{Body, Position},
    game::{Game, PlayerData, Update},
    inputs::Input,
    map::Map,
};

const TILE_SIZE: f32 = 16.0;
const FONT_NAME: &str = "VT323-Regular.ttf";

pub struct Engine {
    width: i32,
    height: i32,
}

impl Engine {
    pub fn new(width: i32, height: i32) -> Self {
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
                .with_resizable(true)
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

        window.set_visible(true);

        let mut redraw = true;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                if redraw {
                    canvas
                        .draw(|graphics| {
                            render_game(&mut game, graphics, &mut font);
                        })
                        .unwrap();
                } else {
                    canvas.redraw_last().unwrap();
                }
            }

            if input_helper.update(&event) {
                if input_helper.quit() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                if let Some(size) = input_helper.window_resized() {
                    canvas.set_size(size.width, size.height);

                    redraw = true;
                } else {
                    match game.update(Input::from_input_helper(&input_helper)) {
                        Update::Refresh => {
                            redraw = true;
                        }
                        Update::Exit => {
                            *control_flow = ControlFlow::Exit;
                        }
                        Update::NoOp => {
                            redraw = false;
                        }
                    }
                }

                if input_helper.key_pressed(VirtualKeyCode::P) {
                    pollster::block_on(canvas.screenshot("rogue.png")).unwrap();
                }

                window.request_redraw();
            }
        });
    }

    fn calculate_translation(game: &Game, graphics: &Graphics) -> (i32, i32) {
        let map = game.world.get_resource::<Map>().unwrap();
        let map_width = map.width;
        let map_height = map.height;

        let player_data = game.world.get_resource::<PlayerData>().unwrap();
        let position = player_data.position;
        let player_x = position.x;
        let player_y = position.y;

        let Size {
            width: canvas_width,
            height: canvas_height,
        } = graphics.size();
        let canvas_width = canvas_width as i32 / TILE_SIZE as i32;
        let canvas_height = canvas_height as i32 / TILE_SIZE as i32;

        let dx = if map_width < canvas_width {
            (canvas_width - map_width) / 2
        } else {
            let dx = canvas_width / 2 - player_x;

            dx.min(0).max(canvas_width - map_width)
        };
        let dy = if map_height < canvas_height {
            (canvas_height - map_height) / 2
        } else {
            let dy = canvas_height / 2 - player_y;

            dy.min(0).max(canvas_height - map_height)
        };
        (dx, dy)
    }
}

fn render_game(game: &mut Game, graphics: &mut Graphics, font: &mut Font) {
    let (dx, dy) = Engine::calculate_translation(game, graphics);
    let dx = dx as f32 * TILE_SIZE;
    let dy = dy as f32 * TILE_SIZE;

    graphics.with_translate(tiefring::Position { x: dx, y: dy }, |graphics| {
        game.world.resource_scope(|_world, map: Mut<Map>| {
            for (j, lines) in map.lines().enumerate() {
                for (i, tile) in lines.iter().enumerate() {
                    if map.is_revealed(i as i32, j as i32) {
                        let rect = Rect::from_xywh(
                            i as f32 * TILE_SIZE,
                            j as f32 * TILE_SIZE,
                            TILE_SIZE,
                            TILE_SIZE,
                        );
                        graphics.draw_rect(rect, tile.color);

                        if !map.is_visible(i as i32, j as i32) {
                            graphics.draw_rect(rect, Color::rgba(0.0, 0.0, 0.0, 0.5));
                        }
                    }
                }
            }
        });
        let mut query = game.world.query::<(&Body, &Position)>();
        query.for_each(&game.world, |(body, position)| {
            body.render(graphics, position, font);
        });
    });
}

impl Body {
    fn render(&self, graphics: &mut Graphics, position: &Position, font: &mut Font) {
        draw_char(self.char, self.color, graphics, position, font);
    }
}

fn draw_char(
    character: char,
    color: Color,
    graphics: &mut Graphics,
    position: &Position,
    font: &mut Font,
) {
    let position =
        tiefring::Position::new(position.x as f32 * TILE_SIZE, position.y as f32 * TILE_SIZE);
    graphics.draw_text(
        font,
        character.to_string(),
        TILE_SIZE as u32,
        position,
        color,
    );
}
