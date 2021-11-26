use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use rand::Rng;
use tiefring::{Canvas, CanvasSettings, Color, Graphics, Rect};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

const WIDTH: u8 = 30;
const HEIGHT: u8 = 20;
const GRID_STEP: f32 = 25.0;

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
            index = (index + 1) % (width * height) as i32;
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

struct Game {
    size: (u8, u8),
    snake: Snake,
    food: Food,
    dt: Duration,
    score: u32,
    pending_direction: Option<Direction>,
}

impl Game {
    fn new((width, height): (u8, u8)) -> Self {
        let snake = Snake::new(width as i32 / 2, height as i32 / 2);
        let food = Food::generate_food(width, height, &snake);
        let dt = Duration::new(0, 0);
        let score = 0;
        let pending_direction = None;

        Game {
            size: (width, height),
            snake,
            food,
            dt,
            score,
            pending_direction,
        }
    }

    fn render(&self, graphics: &mut Graphics) {
        self.snake.render(graphics);
        self.food.render(graphics);
    }

    fn generate_food(&mut self) {
        self.food = Food::generate_food(self.size.0, self.size.1, &self.snake);
    }

    fn try_update(&mut self, dt: Duration, direction: Option<Direction>) -> bool {
        let step = Duration::new(0, 250_000_000);
        if direction.is_some() {
            self.pending_direction = direction;
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
            self.update(self.pending_direction);
        }

        should_update
    }

    fn update(&mut self, direction: Option<Direction>) {
        self.snake.update(&self.food, direction);

        if self.snake.is_eating(&self.food) {
            self.generate_food();
            self.score += 1;
        }

        self.pending_direction = None;
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

    let mut game = Game::new((WIDTH, HEIGHT));

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
            let direction = if input.key_held(VirtualKeyCode::Up) {
                Some(Direction::Up)
            } else if input.key_held(VirtualKeyCode::Left) {
                Some(Direction::Left)
            } else if input.key_held(VirtualKeyCode::Down) {
                Some(Direction::Down)
            } else if input.key_held(VirtualKeyCode::Right) {
                Some(Direction::Right)
            } else {
                None
            };

            if let Some(size) = input.window_resized() {
                canvas.resize(size.width, size.height);
            }

            let now = Instant::now();
            let dt = now.duration_since(time);
            time = now;

            game.try_update(dt, direction);

            window.request_redraw();
        }
    });
}
