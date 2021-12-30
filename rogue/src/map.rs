use std::vec;

use rand::Rng;
use tiefring::Color;
use torchbearer::Map as FovMap;

use crate::components::Position;

#[derive(Clone, Copy)]
pub struct Tile {
    pub walkable: bool,
    pub transparent: bool,
    pub color: Color,
}

impl Tile {
    pub fn wall() -> Self {
        Self {
            walkable: false,
            transparent: false,
            color: Color::rgb(0.3, 0.2, 0.2),
        }
    }

    pub fn floor() -> Self {
        Self {
            walkable: true,
            transparent: true,
            color: Color::rgb(0.5, 0.5, 0.5),
        }
    }
}

pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
    revealed_tiles: Vec<bool>,
    visible_tiles: Vec<bool>,
}

impl Map {
    pub fn empty(width: i32, height: i32) -> Self {
        if width < 1 || height < 1 {
            panic!("Map dimension should be minimum 1x1");
        }

        let tile_count = (width * height) as usize;
        let tiles = vec![Tile::floor(); tile_count];
        let revealed_tiles = vec![false; tile_count];
        let visible_tiles = vec![false; tile_count];

        Self {
            width,
            height,
            tiles,
            revealed_tiles,
            visible_tiles,
        }
    }

    pub fn surround(mut self) -> Self {
        for x in 0..self.width {
            let (a, b) = (self.index(x, 0), self.index(x, self.height - 1));
            self.tiles[a] = Tile::wall();
            self.tiles[b] = Tile::wall();
        }

        for y in 0..self.height {
            let (a, b) = (self.index(0, y), self.index(self.width - 1, y));
            self.tiles[a] = Tile::wall();
            self.tiles[b] = Tile::wall();
        }

        self
    }

    pub fn random_walls(mut self) -> Self {
        let number_of_walls = self.width * self.height / 100;
        let mut rand = rand::thread_rng();
        for _ in 0..number_of_walls {
            let index = rand.gen_range(0..self.tiles.len());
            self.tiles[index] = Tile::wall();
        }

        self
    }

    pub fn lines(&self) -> std::slice::Chunks<Tile> {
        self.tiles.chunks(self.width as usize)
    }

    pub fn reset_visible(&mut self) {
        for tile in self.visible_tiles.iter_mut() {
            *tile = false;
        }
    }
    pub fn reveal(&mut self, positions: &[Position]) {
        for position in positions {
            let index = self.index_from_position(position);
            self.revealed_tiles[index] = true;
            self.visible_tiles[index] = true;
        }
    }

    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        let index = self.index(x, y);
        self.visible_tiles[index]
    }

    pub fn is_revealed(&self, x: i32, y: i32) -> bool {
        let index = self.index(x, y);
        self.revealed_tiles[index]
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (self.width as i32 * y + x) as usize
    }

    fn index_from_position(&self, Position { x, y }: &Position) -> usize {
        (self.width as i32 * y + x) as usize
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        !(x < 0 || y < 0 || x >= self.width || y >= self.height)
    }
}

impl FovMap for Map {
    fn dimensions(&self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }

    fn is_transparent(&self, (x, y): (i32, i32)) -> bool {
        if self.in_bounds(x, y) {
            let index = self.index(x, y);
            self.tiles[index].transparent
        } else {
            false
        }
    }

    fn is_walkable(&self, (x, y): (i32, i32)) -> bool {
        if self.in_bounds(x, y) {
            let index = self.index(x, y);
            self.tiles[index].walkable
        } else {
            false
        }
    }
}
