use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::Add,
    rc::Rc,
    time::{Duration, Instant},
};

use rand::Rng;
use tiefring::{
    sprite::{Sprite, TileSet},
    text::Font,
    Canvas, CanvasSettings, Color, Graphics, Rect, Size,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;
use Option;

const WIDTH: u8 = 30;
const HEIGHT: u8 = 20;
const GRID_STEP: f32 = 32.0;

pub enum Input {
    Up,
    Right,
    Down,
    Left,
    Space,
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

#[derive(PartialEq, Clone, Debug)]
struct Position {
    x: i32,
    y: i32,
}

impl Position {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn moved(&self, direction: Direction) -> Self {
        let (x, y) = match direction {
            Direction::Up => (self.x, self.y - 1),
            Direction::Right => (self.x + 1, self.y),
            Direction::Down => (self.x, self.y + 1),
            Direction::Left => (self.x - 1, self.y),
        };

        Position { x, y }
    }
}

struct Food {
    position: Position,
}

impl Food {
    fn generate_food(width: usize, height: usize, snake: &Snake) -> Self {
        let mut index =
            rand::thread_rng().gen_range(0..width as i32 * height as i32 - snake.body.len() as i32);
        let body_indices: Vec<i32> = snake
            .body
            .iter()
            .map(
                |&Segment {
                     position: Position { x, y },
                     ..
                 }| y * width as i32 + x,
            )
            .collect();
        while body_indices.iter().any(|&i| i == index) {
            index = (index + 1) % (width as i32 * height as i32);
        }

        Food {
            position: Position {
                x: index % width as i32,
                y: index / width as i32,
            },
        }
    }

    fn render(&self, graphics: &mut Graphics) {
        static GREEN: Color = Color {
            r: 0.0,
            g: 0.35,
            b: 0.31,
            a: 1.0,
        };

        let rect = Rect::square(
            (self.position.x as f32 + 0.2) * GRID_STEP,
            (self.position.y as f32 + 0.2) * GRID_STEP,
            GRID_STEP * 0.6,
        );

        graphics.draw_rect(rect, GREEN);
    }
}

#[derive(Clone, Debug)]
struct Snake {
    body: VecDeque<Segment>,
    direction: Direction,
}

impl Snake {
    fn new(x: i32, y: i32) -> Self {
        let mut body = VecDeque::new();
        body.push_back(Segment::new(x, y));
        body.push_back(Segment::new(x - 1, y));

        let direction = Direction::Right;

        Snake { body, direction }
    }

    fn head(&self) -> &Segment {
        self.body.front().expect("The snake has not body")
    }

    fn is_eating_itself(&self) -> bool {
        let head = self.head().clone();
        self.body
            .iter()
            .skip(1)
            .any(|ring| head.position == ring.position)
    }

    fn is_out_of_bounds(&self, bounds: (usize, usize)) -> bool {
        let width = bounds.0 as i32;
        let height = bounds.1 as i32;
        let &Position { x, y } = &self.head().position;
        if x < 0 || x >= width || y < 0 || y >= height {
            true
        } else {
            false
        }
    }

    fn update(&mut self, food: &Food, new_direction: Option<Direction>) {
        if let Some(direction) = new_direction {
            let valid_direction = match (self.direction, direction) {
                (Direction::Up, Direction::Down) => false,
                (Direction::Down, Direction::Up) => false,
                (Direction::Left, Direction::Right) => false,
                (Direction::Right, Direction::Left) => false,
                _ => true,
            };
            if valid_direction {
                self.direction = direction;
            }
        }

        let new_head = self.head().moved(self.direction);
        self.body.push_front(new_head);
        if !self.is_eating(food) {
            self.body.pop_back();
        }
    }

    fn is_eating(&self, food: &Food) -> bool {
        &self.head().position == &food.position
    }

    fn render(&self, graphics: &mut Graphics) {
        static RED: Color = Color {
            r: 0.9,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        };
        static ORANGE: Color = Color {
            r: 0.9,
            g: 0.7,
            b: 0.1,
            a: 1.0,
        };

        let count = self.body.len();
        let step = 0.1 / count as f32;
        let mut squares: VecDeque<Rect> = self
            .body
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let Position { x, y } = segment.position;
                let step = step * index as f32;
                Rect::square(
                    (x as f32 + 0.05 + step) * GRID_STEP,
                    (y as f32 + 0.05 + step) * GRID_STEP,
                    GRID_STEP * (0.9 - 2.0 * step),
                )
            })
            .collect();

        // graphics.draw_rect(
        //     squares
        //         .pop_front()
        //         .expect("A snake should have a head")
        //         .clone(),
        //     RED,
        // );

        // for rect in squares.into_iter() {
        //     graphics.draw_rect(rect, ORANGE)
        // }

        let count = (squares.len() - 1) as f64;
        for (index, rect) in squares.into_iter().enumerate() {
            let percent = index as f64 / count;
            graphics.draw_rect(rect, RED.interpolate(&ORANGE, percent));
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
struct Segment {
    position: Position,
}

impl Segment {
    fn new(x: i32, y: i32) -> Self {
        Self {
            position: Position::new(x, y),
        }
    }

    fn moved(&self, direction: Direction) -> Self {
        Self {
            position: self.position.moved(direction),
        }
    }
}

#[derive(Clone)]
struct Terrain {
    size: (usize, usize),
    tiles: Vec<usize>,
}

impl Terrain {
    fn new((width, height): (usize, usize), grass: &TileSet) -> Self {
        let (tile_count, _) = grass.tile_count();
        let tile_count = tile_count as usize;
        let mut rng = rand::thread_rng();
        let capacity = width as usize * height as usize;

        let tiles = (0..capacity)
            .into_iter()
            .map(|_| rng.gen_range(0..tile_count))
            .collect();

        Self {
            size: (width, height),
            tiles,
        }
    }

    fn render(&self, graphics: &mut Graphics, sprites: &Sprites) {
        let (tile_count, _) = sprites.grass.tile_count();
        let grasses: Vec<_> = (0..tile_count)
            .map(|index| sprites.grass.sprite(index, 0))
            .collect();
        for i in 0..self.size.0 {
            for j in 0..self.size.1 {
                let sprite_index = i as usize + j as usize * self.size.0 as usize;
                graphics.draw_sprite(
                    unsafe { grasses.get_unchecked(*self.tiles.get_unchecked(sprite_index)) },
                    tiefring::Position {
                        top: j as f32 * GRID_STEP,
                        left: i as f32 * GRID_STEP,
                    },
                )
            }
        }
    }
}

enum State {
    Playing,
    Losing {
        score: u32,
        snake: Snake,
        terrain: Terrain,
    },
    Starting,
}

struct Sprites {
    start: Sprite,
    grass: TileSet,
    font: Font,
}

impl Sprites {
    fn new(canvas: &mut Canvas) -> Self {
        let sprites = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("snake/sprites")
            .unwrap();
        let fonts = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("snake/fonts")
            .unwrap();
        Self {
            start: Sprite::load_image(canvas, sprites.join("start.png")).unwrap(),
            grass: TileSet::load_image(
                canvas,
                sprites.join("grass.png"),
                Size::new(GRID_STEP as u32, GRID_STEP as u32),
            )
            .unwrap(),
            font: Font::load_font(fonts.join("VT323-Regular.ttf")).unwrap(),
        }
    }
}

trait Scene {
    fn render(&mut self, graphics: &mut Graphics);
    fn update(&mut self, dt: Duration, input: Option<Input>) -> Option<State>;
}

struct StartingScene {
    size: (u8, u8),
    sprites: Rc<RefCell<Sprites>>,
    terrain: Terrain,
}

impl StartingScene {
    fn new((width, height): (u8, u8), sprites: Rc<RefCell<Sprites>>) -> Self {
        let terrain = Terrain::new((width as usize, height as usize), &sprites.borrow().grass);
        Self {
            size: (width, height),
            sprites,
            terrain: terrain,
        }
    }
}

impl Scene for StartingScene {
    fn render(&mut self, graphics: &mut Graphics) {
        let sprite_height = self.sprites.borrow().start.dimensions.height as f32;
        self.terrain.render(graphics, &self.sprites.borrow());
        let position = tiefring::Position {
            left: (self.size.0 as f32 * GRID_STEP
                - self.sprites.borrow().start.dimensions.width as f32)
                / 2.0,
            top: (self.size.1 as f32 * GRID_STEP - sprite_height) / 2.0,
        };
        graphics.draw_sprite(&self.sprites.borrow().start, position);
        graphics.draw_text(
            &mut self.sprites.borrow_mut().font,
            "Press space to start",
            40,
            position.translated(0.0, sprite_height),
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        )
    }

    fn update(&mut self, _dt: Duration, input: Option<Input>) -> Option<State> {
        if let Some(Input::Space) = input {
            Some(State::Playing)
        } else {
            None
        }
    }
}

struct PlayingScene {
    size: (usize, usize),
    snake: Snake,
    food: Food,
    dt: Duration,
    score: u32,
    pending_input: Option<Input>,
    sprites: Rc<RefCell<Sprites>>,
    terrain: Terrain,
}

impl PlayingScene {
    fn new((width, height): (u8, u8), sprites: Rc<RefCell<Sprites>>) -> Self {
        let width = width as usize;
        let height = height as usize;
        let snake = Snake::new(width as i32 / 2, height as i32 / 2);
        let food = Food::generate_food(width, height, &snake);
        let dt = Duration::new(0, 0);
        let score = 0;
        let terrain = Terrain::new((width, height), &sprites.borrow().grass);

        Self {
            size: (width, height),
            snake,
            food,
            dt,
            score,
            pending_input: None,
            sprites,
            terrain,
        }
    }

    fn move_snake(&mut self) {
        let direction = match self.pending_input {
            Some(Input::Up) => Some(Direction::Up),
            Some(Input::Down) => Some(Direction::Down),
            Some(Input::Left) => Some(Direction::Left),
            Some(Input::Right) => Some(Direction::Right),
            _ => None,
        };

        self.snake.update(&self.food, direction);

        if self.snake.is_eating(&self.food) {
            self.generate_food();
            self.score += 1;
        }
    }

    fn generate_food(&mut self) {
        self.food = Food::generate_food(self.size.0, self.size.1, &self.snake);
    }

    fn is_loosing(&self) -> bool {
        self.snake.is_eating_itself() || self.snake.is_out_of_bounds(self.size)
    }
}

impl Scene for PlayingScene {
    fn render(&mut self, graphics: &mut Graphics) {
        self.terrain.render(graphics, &*self.sprites.borrow());

        self.snake.render(graphics);
        self.food.render(graphics);
        graphics.draw_text(
            &mut self.sprites.borrow_mut().font,
            format!("Score: {}", self.score),
            32,
            tiefring::Position::new(10.0, 0.0),
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            },
        );
    }

    fn update(&mut self, dt: Duration, input: Option<Input>) -> Option<State> {
        let step = Duration::new(0, 200_000_000);
        if input.is_some() {
            self.pending_input = input;
        }

        self.dt += dt;

        let should_update = if self.dt >= step {
            loop {
                self.dt -= step;
                if self.dt < step {
                    break;
                }
            }
            true
        } else {
            false
        };

        if should_update {
            self.move_snake();
        }

        if self.is_loosing() {
            Some(State::Losing {
                score: self.score,
                snake: self.snake.clone(),
                terrain: self.terrain.clone(),
            })
        } else {
            None
        }
    }
}

struct LosingScene {
    snake: Snake,
    score: u32,
    terrain: Terrain,
    sprites: Rc<RefCell<Sprites>>,
}

impl LosingScene {
    fn new(snake: Snake, score: u32, terrain: Terrain, sprites: Rc<RefCell<Sprites>>) -> Self {
        Self {
            snake,
            score,
            terrain,
            sprites,
        }
    }
}

impl Scene for LosingScene {
    fn render(&mut self, graphics: &mut Graphics) {
        self.terrain.render(graphics, &*self.sprites.borrow());
        self.snake.render(graphics);
        graphics.draw_rect(
            Rect {
                left: 0.0,
                top: 0.0,
                right: self.terrain.size.0 as f32 * GRID_STEP,
                bottom: self.terrain.size.1 as f32 * GRID_STEP,
            },
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.2,
            },
        );

        let position = tiefring::Position {
            left: (self.terrain.size.0 / 3) as f32 * GRID_STEP,
            top: (self.terrain.size.1 / 3) as f32 * GRID_STEP,
        };
        graphics.draw_text(
            &mut self.sprites.borrow_mut().font,
            "You lose!",
            80,
            position,
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
        graphics.draw_text(
            &mut self.sprites.borrow_mut().font,
            format!("You scored {} points", self.score),
            40,
            position.translated(0.0, 80.0),
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
        graphics.draw_text(
            &mut self.sprites.borrow_mut().font,
            "Press space to play again.",
            40,
            position.translated(0.0, 120.0),
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
    }

    fn update(&mut self, _dt: Duration, input: Option<Input>) -> Option<State> {
        // Only need to check if space was pressed.
        if let Some(Input::Space) = input {
            Some(State::Playing)
        } else {
            None
        }
    }
}

struct Game {
    size: (u8, u8),
    scene: Box<dyn Scene>,
    sprites: Rc<RefCell<Sprites>>,
}

impl Game {
    fn new((width, height): (u8, u8), canvas: &mut Canvas) -> Self {
        let sprites = Rc::new(RefCell::new(Sprites::new(canvas)));

        let scene = Box::new(StartingScene::new((width, height), sprites.clone()));

        Game {
            size: (width, height),
            scene,
            sprites,
        }
    }

    fn render(&mut self, graphics: &mut Graphics) {
        self.scene.render(graphics);
    }

    fn update(&mut self, dt: Duration, input: Option<Input>) -> bool {
        let result = self.scene.update(dt, input);
        match result {
            Some(State::Starting) => {
                self.scene = Box::new(StartingScene::new(self.size, self.sprites.clone()));
            }
            Some(State::Playing) => {
                self.scene = Box::new(PlayingScene::new(self.size, self.sprites.clone()));
            }
            Some(State::Losing {
                snake,
                score,
                terrain,
            }) => {
                self.scene = Box::new(LosingScene::new(
                    snake,
                    score,
                    terrain,
                    self.sprites.clone(),
                ))
            }
            None => {}
        };

        true
    }
}

fn main() {
    static GRASS_GREEN: Color = Color {
        r: 0.72,
        g: 0.85,
        b: 0.50,
        a: 1.0,
    };
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(GRID_STEP * WIDTH as f32, GRID_STEP * HEIGHT as f32);
        WindowBuilder::new()
            .with_title("Snaky")
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
            CanvasSettings {
                background_color: GRASS_GREEN,
                ..Default::default()
            },
        ))
    }
    .unwrap();

    let mut game = Game::new((WIDTH, HEIGHT), &mut canvas);

    let mut time = Instant::now();
    window.set_visible(true);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        if let Event::RedrawRequested(_) = event {
            canvas
                .draw(|graphics| {
                    game.render(graphics);
                })
                .unwrap();
        }

        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }
            let keyboard_input = if input.key_held(VirtualKeyCode::Up) {
                Some(Input::Up)
            } else if input.key_held(VirtualKeyCode::Left) {
                Some(Input::Left)
            } else if input.key_held(VirtualKeyCode::Down) {
                Some(Input::Down)
            } else if input.key_held(VirtualKeyCode::Right) {
                Some(Input::Right)
            } else if input.key_pressed(VirtualKeyCode::Space) {
                Some(Input::Space)
            } else {
                None
            };

            if input.key_pressed(VirtualKeyCode::P) {
                pollster::block_on(canvas.screenshot("snake.png")).unwrap();
            }

            if let Some(size) = input.window_resized() {
                canvas.resize(size.width, size.height);
            }

            let now = Instant::now();
            let dt = now.duration_since(time);
            time = now;

            game.update(dt, keyboard_input);

            window.request_redraw();
        }
    });
}

trait Interpolator<Rhs = Self> {
    type Output;

    fn interpolate(&self, other: &Rhs, percent: f64) -> Self::Output;
}

impl Interpolator for Color {
    type Output = Color;

    fn interpolate(&self, other: &Self, percent: f64) -> Self::Output {
        Color {
            r: self.a * (1.0 - percent) + other.r * percent,
            g: self.g * (1.0 - percent) + other.g * percent,
            b: self.b * (1.0 - percent) + other.b * percent,
            a: self.a * (1.0 - percent) + other.a * percent,
        }
    }
}
