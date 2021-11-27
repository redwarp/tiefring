use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use rand::Rng;
use tiefring::{sprite::Sprite, Canvas, CanvasSettings, Color, Graphics, Rect};
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
const GRID_STEP: f32 = 25.0;

pub enum Input {
    Up,
    Right,
    Down,
    Left,
    Space,
}

#[derive(Clone, Copy)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

#[derive(PartialEq)]
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
    fn generate_food(width: u8, height: u8, snake: &Snake) -> Self {
        let mut index =
            rand::thread_rng().gen_range(0..width as i32 * height as i32 - snake.body.len() as i32);
        let body_indices: Vec<i32> = snake
            .body
            .iter()
            .map(|&Position { x, y }| y * width as i32 + x)
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

struct Snake {
    body: VecDeque<Position>,
    direction: Direction,
}

impl Snake {
    fn new(x: i32, y: i32) -> Self {
        let mut body = VecDeque::new();
        body.push_back(Position::new(x, y));
        body.push_back(Position::new(x, y).moved(Direction::Left));

        let direction = Direction::Right;

        Snake { body, direction }
    }

    fn head(&self) -> &Position {
        self.body.front().expect("The snake has not body")
    }

    fn is_eating_itself(&self) -> bool {
        let head = self.head().clone();
        self.body.iter().skip(1).any(|ring| head == ring)
    }

    fn is_out_of_bounds(&self, bounds: (u8, u8)) -> bool {
        let width = bounds.0 as i32;
        let height = bounds.1 as i32;
        let &Position { x, y } = self.head().clone();
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
        self.head() == &food.position
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

        let mut squares: VecDeque<Rect> = self
            .body
            .iter()
            .map(|&Position { x, y }| {
                Rect::square(
                    ((x as f32 + 0.05) * GRID_STEP).ceil(),
                    ((y as f32 + 0.05) * GRID_STEP).ceil(),
                    (GRID_STEP * 0.9).floor(),
                )
            })
            .collect();

        graphics.draw_rect(
            squares
                .pop_front()
                .expect("A snake should have a head")
                .clone(),
            RED,
        );

        for rect in squares.into_iter() {
            graphics.draw_rect(rect, ORANGE)
        }
    }
}

enum State {
    Playing,
    Losing,
    Starting,
}

struct Sprites {
    start_sprite: Sprite,
}

struct Game {
    size: (u8, u8),
    snake: Snake,
    food: Food,
    dt: Duration,
    score: u32,
    pending_input: Option<Input>,
    state: State,
    sprites: Sprites,
}

impl Game {
    fn new((width, height): (u8, u8), canvas: &mut Canvas) -> Self {
        let snake = Snake::new(width as i32 / 2, height as i32 / 2);
        let food = Food::generate_food(width, height, &snake);
        let dt = Duration::new(0, 0);
        let score = 0;

        let sprites = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("snake/sprites")
            .unwrap();
        let sprites = Sprites {
            start_sprite: Sprite::load_image(canvas, sprites.join("start.png")).unwrap(),
        };

        Game {
            size: (width, height),
            snake,
            food,
            dt,
            score,
            pending_input: None,
            state: State::Starting,
            sprites,
        }
    }

    fn render(&self, graphics: &mut Graphics) {
        match self.state {
            State::Starting => {
                let position = tiefring::Position {
                    left: (self.size.0 as f32 * GRID_STEP
                        - self.sprites.start_sprite.dimensions.width as f32)
                        / 2.0,
                    top: (self.size.1 as f32 * GRID_STEP
                        - self.sprites.start_sprite.dimensions.height as f32)
                        / 2.0,
                };
                graphics.draw_sprite(&self.sprites.start_sprite, position);
            }
            State::Playing => {
                self.snake.render(graphics);
                self.food.render(graphics);
            }
            _ => {}
        }
    }

    fn generate_food(&mut self) {
        self.food = Food::generate_food(self.size.0, self.size.1, &self.snake);
    }

    fn is_loosing(&self) -> bool {
        self.snake.is_eating_itself() || self.snake.is_out_of_bounds(self.size)
    }

    fn try_update(&mut self, dt: Duration, input: Option<Input>) -> bool {
        let step = Duration::new(0, 250_000_000);
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
            self.update();
        }

        should_update
    }

    fn update(&mut self) {
        match self.state {
            State::Starting => {
                if let Some(Input::Space) = self.pending_input {
                    self.state = State::Playing;
                }
            }
            State::Playing => {
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
                if self.is_loosing() {
                    self.state = State::Starting;
                }
            }
            State::Losing => {}
        }

        self.pending_input = None;
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

            if let Some(size) = input.window_resized() {
                canvas.resize(size.width, size.height);
            }

            let now = Instant::now();
            let dt = now.duration_since(time);
            time = now;

            game.try_update(dt, keyboard_input);

            window.request_redraw();
        }
    });
}
