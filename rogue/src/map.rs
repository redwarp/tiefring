use std::vec;

use bevy_ecs::prelude::World;
use rand::{prelude::StdRng, Rng, SeedableRng};
use tiefring::Color;
use torchbearer::{fov::VisionMap, path::PathMap};

use crate::{components::Position, spawner};

#[derive(Clone, Copy)]
pub struct Tile {
    pub walkable: bool,
    pub transparent: bool,
    pub color: Color,
    pub tile_type: TileType,
}

impl Tile {
    pub fn wall() -> Self {
        Self {
            walkable: false,
            transparent: false,
            color: Color::rgb(0.3, 0.2, 0.2),
            tile_type: TileType::Wall,
        }
    }

    pub fn floor() -> Self {
        Self {
            walkable: true,
            transparent: true,
            color: Color::rgb(0.5, 0.5, 0.5),
            tile_type: TileType::Floor,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TileType {
    Wall,
    Floor,
}

pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
    revealed_tiles: Vec<bool>,
    visible_tiles: Vec<bool>,
    pub starting_position: Position,
    pub map_representation: MapRepresentation,
    pub blocked: Vec<bool>,
}

impl Map {
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

    pub fn dungeon(width: i32, height: i32, world: &mut World) -> Self {
        if width < 1 || height < 1 {
            panic!("Map dimension should be minimum 1x1");
        }

        let tile_count = (width * height) as usize;
        let tiles = vec![Tile::wall(); tile_count];

        let revealed_tiles = vec![false; tile_count];
        let visible_tiles = vec![false; tile_count];
        let starting_position = Position::new(0, 0);
        let map_representation = MapRepresentation::with_capacity(0);
        let blocked = vec![false; tile_count];

        let mut map = Self {
            width,
            height,
            tiles,
            revealed_tiles,
            visible_tiles,
            starting_position,
            map_representation,
            blocked,
        };

        let max_room = (width * height) / 100;
        const ROOM_MAX_SIZE: i32 = 10;
        const ROOM_MIN_SIZE: i32 = 6;
        let mut rng = StdRng::seed_from_u64(42);

        let mut rooms: Vec<Room> = vec![];
        for _ in 0..max_room {
            let room_width = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);
            let room_height = rng.gen_range(ROOM_MIN_SIZE..=ROOM_MAX_SIZE);
            let x = rng.gen_range(0..width - room_width);
            let y = rng.gen_range(0..height - room_height);
            let new_room = Room::new(x, y, room_width, room_height);

            if !rooms.iter().any(|room| new_room.overlaps(room)) {
                rooms.push(new_room);
            }
        }

        for (index, room) in rooms.iter().enumerate() {
            apply_room(room, &mut map);

            let (new_x, new_y) = room.center();
            if index == 0 {
                map.starting_position.x = new_x;
                map.starting_position.y = new_y;
            } else {
                let (prev_x, prev_y) = rooms[index - 1].center();

                if rng.gen::<bool>() {
                    create_horizontal_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_vertical_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    create_vertical_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_horizontal_tunnel(prev_x, new_x, new_y, &mut map)
                }
            }
        }

        spawn_monsters(&rooms, &mut rng, world);
        map.map_representation = MapRepresentation::new(width, height, &map.tiles);
        map.reset_blocked();

        map
    }

    pub fn tile_index_at_position(&self, x: i32, y: i32) -> Option<&usize> {
        self.map_representation
            .tiles_index
            .get(index_with_width(self.width, x, y))
    }

    pub fn reset_visible(&mut self) {
        for tile in self.visible_tiles.iter_mut() {
            *tile = false;
        }
    }

    pub fn reset_blocked(&mut self) {
        for (index, tile) in self.tiles.iter().enumerate() {
            self.blocked[index] = tile.walkable == false;
        }
    }

    pub fn index(&self, x: i32, y: i32) -> usize {
        index_with_width(self.width, x, y)
    }

    pub fn index_from_position(&self, &Position { x, y }: &Position) -> usize {
        index_with_width(self.width, x, y)
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        !(x < 0 || y < 0 || x >= self.width || y >= self.height)
    }
}

impl VisionMap for Map {
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
}

impl PathMap for Map {
    fn dimensions(&self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }

    fn is_walkable(&self, (x, y): (i32, i32)) -> bool {
        if self.in_bounds(x, y) {
            let index = self.index(x, y);
            !self.blocked[index]
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct Room {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Room {
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    fn overlaps(&self, other: &Room) -> bool {
        self.left <= other.right
            && self.right >= other.left
            && self.top <= other.bottom
            && self.bottom >= other.top
    }

    fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
}

#[inline(always)]
fn index_with_width(width: i32, x: i32, y: i32) -> usize {
    (width as i32 * y + x) as usize
}

fn position_with_index(index: usize, width: i32) -> (i32, i32) {
    let index = index as i32;
    let x = index % width;
    let y = index / width;
    (x, y)
}

fn apply_room(room: &Room, map: &mut Map) {
    for y in room.top + 1..=room.bottom - 1 {
        for x in room.left + 1..=room.right - 1 {
            let index = map.index(x, y);
            map.tiles[index] = Tile::floor();
        }
    }
}

fn create_horizontal_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in x1.min(x2)..(x1.max(x2) + 1) {
        map.tiles[x as usize + y as usize * map.width as usize] = Tile::floor();
    }
}
fn create_vertical_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in y1.min(y2)..(y1.max(y2) + 1) {
        map.tiles[x as usize + y as usize * map.width as usize] = Tile::floor();
    }
}

fn spawn_monsters(rooms: &[Room], rng: &mut StdRng, world: &mut World) {
    let mut orc_number = 0;
    for room in &rooms[1..] {
        let (x, y) = room.center();
        if rng.gen::<f32>() < 0.8 {
            orc_number += 1;
            spawner::orc(world, format!("{}", orc_number).as_str(), x, y);
        } else {
            spawner::deer(world, x, y);
        }
    }
}

pub struct MapRepresentation {
    tiles_index: Vec<usize>,
}

impl MapRepresentation {
    fn new(width: i32, height: i32, tiles: &[Tile]) -> Self {
        let tiles_index = tiles
            .iter()
            .enumerate()
            .map(|(index, tile)| match tile.tile_type {
                TileType::Floor => 5,
                TileType::Wall => {
                    MapRepresentation::wall_configuration(index, tiles, width, height)
                }
            })
            .collect();
        Self { tiles_index }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            tiles_index: Vec::with_capacity(capacity),
        }
    }

    fn wall_configuration(tile_index: usize, tiles: &[Tile], width: i32, height: i32) -> usize {
        let bitmask = MapRepresentation::wall_bitmask(tile_index, tiles, width, height);
        MapRepresentation::bitmask_to_sprite_index(bitmask)
    }

    fn bitmask_to_sprite_index(bitmask: u32) -> usize {
        match bitmask {
            0b00000000 => 1,
            0b11111000 => 10,
            0b11111111 => 7,
            0b00011111 => 3,
            0b00010110 => 4,
            0b00001011 => 2,
            0b01101000 => 9,
            0b11010000 => 11,
            0b01101011 => 6,
            0b11010110 => 8,
            0b00001000 => 17,
            0b00000010 => 19,
            0b00010000 => 18,
            0b01000000 => 21,
            0b01011010 => 22,
            0b01000010 => 20,
            0b00011000 => 16,
            0b11111110 => 12,
            0b11111011 => 13,
            0b11011111 => 15,
            0b01111111 => 14,
            _ => 1,
        }
    }

    // https://gamedevelopment.tutsplus.com/tutorials/how-to-use-tile-bitmasking-to-auto-tile-your-level-layouts--cms-25673
    fn wall_bitmask(tile_index: usize, tiles: &[Tile], width: i32, height: i32) -> u32 {
        let (x, y) = position_with_index(tile_index, width);
        let neighboors: [(i32, i32); 8] = [
            (x - 1, y - 1),
            (x, y - 1),
            (x + 1, y - 1),
            (x - 1, y),
            (x + 1, y),
            (x - 1, y + 1),
            (x, y + 1),
            (x + 1, y + 1),
        ];
        let values: Vec<u32> = neighboors
            .iter()
            .map(|&(x, y)| {
                if x < 0 || x >= width || y < 0 || y >= height {
                    1
                } else {
                    let index = index_with_width(width, x, y);
                    if tiles[index].tile_type == TileType::Wall {
                        1
                    } else {
                        0
                    }
                }
            })
            .collect();
        let mut bitmask = 0;
        if values[0] == 1 && values[1] == 1 && values[3] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[1] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[2] == 1 && values[1] == 1 && values[4] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[3] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[4] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[5] == 1 && values[3] == 1 && values[6] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[6] == 1 {
            bitmask += 1;
        }
        bitmask = bitmask << 1;
        if values[7] == 1 && values[6] == 1 && values[4] == 1 {
            bitmask += 1;
        }

        bitmask
    }
}

#[cfg(test)]
mod tests {
    use super::{index_with_width, MapRepresentation, Tile};

    #[test]
    fn bitmask() {
        let tiles = vec![Tile::wall(); 9];

        let map_representation = MapRepresentation::new(3, 3, &tiles);
        assert_eq!(map_representation.tiles_index, [7, 7, 7, 7, 7, 7, 7, 7, 7]);
    }

    #[test]
    fn bitmask_floor_bottom() {
        let tiles = [
            Tile::wall(),
            Tile::wall(),
            Tile::wall(),
            Tile::wall(),
            Tile::wall(),
            Tile::wall(),
            Tile::floor(),
            Tile::floor(),
            Tile::floor(),
        ];

        let map_representation = MapRepresentation::new(3, 3, &tiles);
        assert_eq!(
            map_representation.tiles_index,
            [7, 7, 7, 10, 10, 10, 5, 5, 5]
        );
    }

    #[test]
    fn test_index_with_width() {
        let index = index_with_width(80, 50, 0);

        assert_eq!(50, index);
    }
}
