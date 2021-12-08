use tiefring::Color;

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

pub struct LeftMover;
