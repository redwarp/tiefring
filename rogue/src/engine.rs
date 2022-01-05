use anyhow::Result;
use bevy_ecs::prelude::Mut;
use tiefring::{
    sprite::{Sprite, TileSet},
    Canvas, CanvasSettings, Color, Graphics, Rect, SizeInPx,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use crate::{
    components::{Body, BodyType, Position},
    game::{Game, PlayerData, Update},
    inputs::Input,
    map::Map,
};

const TILE_SIZE: f32 = 32.0;

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

        let sprites = Sprites::new(&mut canvas);

        window.set_visible(true);

        let mut redraw = true;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                if redraw {
                    let SizeInPx { width, height } = canvas.size();
                    let cell_count_x = (width as f32 / TILE_SIZE).ceil() as i32;
                    let cell_count_y = (height as f32 / TILE_SIZE).ceil() as i32;

                    canvas
                        .draw(|graphics| {
                            render_game(&mut game, graphics, &sprites, cell_count_x, cell_count_y);
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

        let SizeInPx {
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

fn render_game(
    game: &mut Game,
    graphics: &mut Graphics,
    sprites: &Sprites,
    cell_count_x: i32,
    cell_count_y: i32,
) {
    let (dx, dy) = Engine::calculate_translation(game, graphics);
    let translate_x = dx as f32 * TILE_SIZE;
    let translate_y = dy as f32 * TILE_SIZE;

    graphics.with_translation(
        tiefring::Position {
            x: translate_x,
            y: translate_y,
        },
        |graphics| {
            game.world.resource_scope(|world, map: Mut<Map>| {
                let min_x = (-dx).max(0);
                let min_y = (-dy).max(0);
                let max_x = (min_x + cell_count_x).min(map.width);
                let max_y = (min_y + cell_count_y).min(map.height);

                for j in min_y..max_y {
                    for i in min_x..max_x {
                        let tile_index = map.tile_index_at_position(i, j).unwrap();
                        if map.is_revealed(i as i32, j as i32) {
                            let rect = Rect::from_xywh(
                                i as f32 * TILE_SIZE,
                                j as f32 * TILE_SIZE,
                                TILE_SIZE,
                                TILE_SIZE,
                            );
                            graphics.draw_sprite_in_rect(
                                sprites.tiles.sprite_with_index(*tile_index),
                                rect,
                            );

                            if !map.is_visible(i as i32, j as i32) {
                                graphics.draw_rect(rect, Color::rgba(0.0, 0.0, 0.05, 0.8));
                            }
                        }
                    }
                }

                let mut query = world.query::<(&Body, &Position)>();
                query.for_each(world, |(body, position)| {
                    if map.is_visible(position.x, position.y) {
                        body.render(graphics, position, sprites);
                    }
                });
            });
        },
    );
}

impl Body {
    fn render(&self, graphics: &mut Graphics, position: &Position, sprites: &Sprites) {
        let position =
            tiefring::Position::new(position.x as f32 * TILE_SIZE, position.y as f32 * TILE_SIZE);
        graphics.draw_sprite(sprites.sprite(&self.body_type), position);
    }
}

struct Sprites {
    tiles: TileSet,
    people: TileSet,
}

impl Sprites {
    fn new(canvas: &mut Canvas) -> Self {
        let sprites = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("rogue/sprites")
            .unwrap();

        let tiles = TileSet::load_image(canvas, sprites.join("tiles.png"), (32, 32)).unwrap();
        let people = TileSet::load_image(canvas, sprites.join("chars.png"), (32, 32)).unwrap();
        Self { tiles, people }
    }

    fn sprite(&self, character: &BodyType) -> &Sprite {
        match character {
            BodyType::Hero => self.people.sprite(0, 0),
            BodyType::Orc => self.people.sprite(1, 0),
            BodyType::Deer => self.people.sprite(2, 0),
            BodyType::BonePile => self.people.sprite(0, 1),
        }
    }
}
