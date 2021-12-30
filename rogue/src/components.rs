use tiefring::Color;

pub struct Name(pub String);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
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
