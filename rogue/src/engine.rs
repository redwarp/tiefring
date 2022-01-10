use std::{cell::RefCell, cmp::Ordering, rc::Rc};

use anyhow::Result;
use bevy_ecs::prelude::{Entity, Mut, World};
use tiefring::{
    sprite::{Sprite, TileSet},
    text::Font,
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
    components::{Body, BodyType, Health, Position, Solid},
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

        let mut renderer = Renderer::new(&mut canvas);

        window.set_visible(true);

        let mut redraw = true;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            if let Event::RedrawRequested(_) = event {
                if redraw {
                    renderer.update(&mut game);
                    canvas
                        .draw(|graphics| {
                            renderer.render_game(&mut game, graphics);
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
}

struct Renderer {
    sprites: Sprites,
    hud: Hud,
}

impl Renderer {
    fn new(canvas: &mut Canvas) -> Self {
        let sprites = Sprites::new(canvas);
        let fonts = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("resources/fonts")
            .unwrap();
        let hud = {
            let font = Rc::new(RefCell::new(
                Font::load_font(fonts.join("VT323-Regular.ttf")).unwrap(),
            ));

            Hud::new(font)
        };

        Self { sprites, hud }
    }

    fn update(&mut self, game: &mut Game) {
        self.hud.update(game);
    }

    fn render_game(&self, game: &mut Game, graphics: &mut Graphics) {
        let SizeInPx { width, height } = graphics.size();
        let cell_count_x = (width as f32 / TILE_SIZE).ceil() as i32;
        let cell_count_y = (height as f32 / TILE_SIZE).ceil() as i32;

        let (dx, dy) = Renderer::calculate_translation_in_tiles(game, graphics);
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

                    let x_range = min_x..max_x;
                    let y_range = min_y..max_y;

                    for j in y_range.clone() {
                        for i in x_range.clone() {
                            let tile_index = map.tile_index_at_position(i, j).unwrap();
                            if map.is_revealed(i as i32, j as i32) {
                                let rect = Rect::from_xywh(
                                    i as f32 * TILE_SIZE,
                                    j as f32 * TILE_SIZE,
                                    TILE_SIZE,
                                    TILE_SIZE,
                                );
                                graphics.draw_sprite_in_rect(
                                    self.sprites.tiles.sprite_with_index(*tile_index),
                                    rect,
                                );
                            }
                        }
                    }

                    // Separate drawing the fog of war overlay from sprites, to reduce draw calls.
                    for j in y_range.clone() {
                        for i in x_range.clone() {
                            if map.is_revealed(i as i32, j as i32) {
                                let rect = Rect::from_xywh(
                                    i as f32 * TILE_SIZE,
                                    j as f32 * TILE_SIZE,
                                    TILE_SIZE,
                                    TILE_SIZE,
                                );
                                if !map.is_visible(i as i32, j as i32) {
                                    graphics.draw_rect(rect, Color::rgba(0.0, 0.0, 0.05, 0.8));
                                }
                            }
                        }
                    }

                    let mut query = world.query::<(&Body, &Position, Option<&Solid>)>();
                    let mut bodies: Vec<_> = query.iter(world).collect();
                    bodies.sort_by(|a, b| {
                        if a.2.is_some() {
                            Ordering::Greater
                        } else if b.2.is_some() {
                            Ordering::Less
                        } else {
                            Ordering::Equal
                        }
                    });

                    for (body, position, _) in bodies {
                        if x_range.contains(&position.x)
                            && y_range.contains(&position.y)
                            && map.is_visible(position.x, position.y)
                        {
                            body.render(graphics, position, &self.sprites);
                        }
                    }
                });
            },
        );

        self.hud.render(graphics);
    }

    fn calculate_translation_in_tiles(game: &Game, graphics: &Graphics) -> (i32, i32) {
        let map = game.world.get_resource::<Map>().unwrap();
        let map_width = map.width;
        let map_height = map.height;

        let player_position = game
            .world
            .get_resource::<PlayerData>()
            .expect("PlayerData is contained by world")
            .position;

        let SizeInPx {
            width: canvas_width,
            height: canvas_height,
        } = graphics.size();
        let canvas_width = canvas_width as i32 / TILE_SIZE as i32;
        let canvas_height = canvas_height as i32 / TILE_SIZE as i32;

        let dx = if map_width < canvas_width {
            (canvas_width - map_width) / 2
        } else {
            let dx = canvas_width / 2 - player_position.x;

            dx.clamp(canvas_width - map_width, 0)
        };
        let dy = if map_height < canvas_height {
            (canvas_height - map_height) / 2
        } else {
            let dy = canvas_height / 2 - player_position.y;

            dy.clamp(canvas_height - map_height, 0)
        };
        (dx, dy)
    }
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

struct Hud {
    health_bar: StatBar,
}

impl Hud {
    fn new(font: Rc<RefCell<Font>>) -> Self {
        let mut health_bar = StatBar::new("Health", 0, Color::rgb(0.8, 0.0, 0.0), font);
        health_bar.current = 12;

        Self { health_bar }
    }

    fn update(&mut self, game: &mut Game) {
        let player_entity = game.world.player_entity();
        let mut query = game.world.query::<&Health>();
        if let Ok(health) = query.get(&game.world, player_entity) {
            self.health_bar.update(health.hp, health.max_hp);
        }
    }

    fn render(&self, graphics: &mut Graphics) {
        self.health_bar.render(graphics, Position::new(0, 0));
    }
}

struct StatBar {
    name: String,
    current: i32,
    max: i32,
    color: Color,
    font: Rc<RefCell<Font>>,
}

impl StatBar {
    fn new(name: &str, max: i32, color: Color, font: Rc<RefCell<Font>>) -> Self {
        Self {
            name: name.to_string(),
            current: max,
            max,
            color,
            font,
        }
    }

    fn update(&mut self, current: i32, max: i32) {
        self.current = current.max(0);
        self.max = max;
    }

    fn render(&self, graphics: &mut Graphics, origin: Position) {
        if self.max <= 0 {
            return;
        }

        const WIDTH: f32 = 150.0;
        const HEIGHT: f32 = 20.0;

        let text = format!("{} ({}/{})", self.name, self.current, self.max);
        let origin_x = origin.x as f32 * TILE_SIZE + 10.0;
        let origin_y = origin.y as f32 * TILE_SIZE + 10.0;
        let ratio = self.current as f32 / self.max as f32;
        let rect = Rect::from_xywh(origin_x, origin_y, WIDTH * ratio, HEIGHT);

        graphics.draw_rect(rect, self.color);

        let rect = Rect::from_xywh(
            origin_x + WIDTH * ratio,
            origin_y,
            WIDTH * (1.0 - ratio),
            HEIGHT,
        );

        graphics.draw_rect(rect, darker(&self.color));

        graphics.draw_text(
            &mut self.font.borrow_mut(),
            text,
            HEIGHT as u32,
            tiefring::Position::new(origin_x + WIDTH + 10.0, origin_y),
            Color::rgb(1.0, 1.0, 1.0),
        )
    }
}

pub trait CommonData {
    fn player_data(&self) -> &PlayerData;
    fn player_entity(&self) -> Entity;
}

impl CommonData for World {
    fn player_data(&self) -> &PlayerData {
        self.get_resource::<PlayerData>().unwrap()
    }

    fn player_entity(&self) -> Entity {
        self.get_resource::<PlayerData>().unwrap().entity
    }
}

pub fn darker(color: &Color) -> Color {
    Color {
        a: color.a,
        r: color.r * 0.75,
        g: color.g * 0.75,
        b: color.b * 0.75,
    }
}
