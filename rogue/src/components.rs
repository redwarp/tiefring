use tiefring::Color;

pub struct Name(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        ((self.x - other.x).pow(2) as f32 + (self.y - other.y).pow(2) as f32).sqrt()
    }
}

pub struct Body {
    pub char: char,
    pub color: Color,
}

impl Body {
    pub fn new(char: char, color: Color) -> Self {
        Self { char, color }
    }
}

pub struct Player;

pub struct Monster;

pub struct MoveRandom;

pub struct MoveClose;

pub struct Vision {
    pub visible_positions: Vec<Position>,
    pub view_distance: i32,
}

impl Vision {
    pub fn new(view_distance: i32) -> Self {
        Self {
            visible_positions: Vec::new(),
            view_distance,
        }
    }
}

pub struct Solid;

pub struct Health {
    hp: i32,
    max_hp: i32,
}

impl Health {
    pub fn full_health(max_hp: i32) -> Self {
        Self { hp: max_hp, max_hp }
    }
}
