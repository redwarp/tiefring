use bevy_ecs::prelude::Component;

#[derive(Component)]
pub struct Name(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[allow(dead_code)]
    pub fn distance_to(&self, other: &Position) -> f32 {
        ((self.x - other.x).pow(2) as f32 + (self.y - other.y).pow(2) as f32).sqrt()
    }
}

#[derive(Component)]
pub struct Body {
    pub body_type: BodyType,
}

impl Body {
    pub fn new(body_type: BodyType) -> Self {
        Self { body_type }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BodyType {
    Hero,
    Orc,
    Deer,
    BonePile,
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Monster;

#[derive(Component)]
pub struct MoveRandom;

#[derive(Component)]
pub struct MoveClose;

#[derive(Component)]
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

#[derive(Component)]
pub struct Solid;

#[derive(Component)]
pub struct Health {
    pub hp: i32,
    pub max_hp: i32,
}

impl Health {
    pub fn full_health(max_hp: i32) -> Self {
        Self { hp: max_hp, max_hp }
    }
}

#[derive(Component)]
pub struct Stats {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub magic: i32,
}

impl Stats {
    pub fn new(strength: i32, dexterity: i32, constitution: i32, magic: i32) -> Self {
        Self {
            strength,
            dexterity,
            constitution,
            magic,
        }
    }

    pub fn max_health(&self) -> i32 {
        10 + self.constitution * 2
    }
}
