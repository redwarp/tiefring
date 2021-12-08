use tiefring::Color;

#[derive(Clone, Copy)]
pub struct Tile {
    pub walkable: bool,
    pub color: Color,
}

impl Tile {
    pub fn wall() -> Self {
        Self {
            walkable: false,
            color: Color::rgb(0.1, 0.1, 0.1),
        }
    }

    pub fn floor() -> Self {
        Self {
            walkable: true,
            color: Color::rgb(0.4, 0.4, 0.4),
        }
    }
}

pub struct Map {
    pub width: u32,
    pub height: u32,
    tiles: Vec<Tile>,
}

impl Map {
    pub fn empty(width: u32, height: u32) -> Self {
        let tiles = vec![Tile::floor(); (width * height) as usize];
        Self {
            width,
            height,
            tiles,
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

    pub fn lines(&self) -> std::slice::Chunks<Tile> {
        self.tiles.chunks(self.width as usize)
    }

    fn index(&self, x: u32, y: u32) -> usize {
        (self.width * y + x) as usize
    }
}
