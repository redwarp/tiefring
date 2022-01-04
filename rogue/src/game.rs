use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ShouldRun;
use bevy_ecs::{
    prelude::World,
    schedule::{Schedule, SystemStage},
};
use rand::prelude::StdRng;
use rand::SeedableRng;
use torchbearer::path::PathMap;

use crate::components::{Player, Position};
use crate::map::Map;
use crate::spawner;
use crate::{inputs::Input, systems};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RunState {
    AiTurn,
    WaitingForInput,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Update {
    Refresh,
    Exit,
    NoOp,
}

pub struct Game {
    pub world: World,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, StageLabel)]
enum Stages {
    Update,
    MapData,
    Monster,
}

fn ai_turn(run_state: Res<RunState>) -> ShouldRun {
    match *run_state {
        RunState::AiTurn => ShouldRun::Yes,
        _ => ShouldRun::No,
    }
}

impl Game {
    pub fn new(width: i32, height: i32) -> Self {
        let mut schedule = Schedule::default();

        schedule
            .add_stage(
                Stages::Monster,
                SystemStage::parallel()
                    .with_run_criteria(ai_turn.system())
                    .with_system(systems::move_random.system())
                    .with_system(systems::move_close.system())
                    .with_system(systems::insult.system()),
            )
            .add_stage_after(
                Stages::Monster,
                Stages::MapData,
                SystemStage::parallel()
                    .with_system(systems::update_visible.system())
                    .with_system(systems::update_blocked.system()),
            )
            .add_stage_after(
                Stages::MapData,
                Stages::Update,
                SystemStage::parallel().with_system(systems::field_of_view.system()),
            );

        let mut world = World::new();
        let map = Map::dungeon(width, height, &mut world);

        let starting_position = map.starting_position;
        world.insert_resource(map);
        let rng = StdRng::from_entropy();
        world.insert_resource(rng);

        let player = spawner::player(&mut world, starting_position.x, starting_position.y);
        let player_data = PlayerData {
            entity: player,
            position: starting_position,
        };
        world.insert_resource(player_data);
        // Run the schedule work once to update initial field of view.
        world.insert_resource::<RunState>(RunState::WaitingForInput);
        schedule.run(&mut world);
        world.insert_resource::<RunState>(RunState::AiTurn);

        Self { world, schedule }
    }

    pub fn update(&mut self, input: Option<Input>) -> Update {
        let run_state: &RunState = self.world.get_resource().unwrap();

        match run_state {
            RunState::AiTurn => {
                self.schedule.run(&mut self.world);
                self.world
                    .insert_resource::<RunState>(RunState::WaitingForInput);
                Update::Refresh
            }
            RunState::WaitingForInput => {
                if let Some(Input::Escape) = input {
                    Update::Exit
                } else if self.try_move_player(&input) {
                    self.schedule.run(&mut self.world);
                    self.world.insert_resource::<RunState>(RunState::AiTurn);
                    Update::NoOp
                } else {
                    Update::NoOp
                }
            }
        }
    }

    fn try_move_player(&mut self, input: &Option<Input>) -> bool {
        match input {
            Some(Input::Up) => self.move_player(0, -1),
            Some(Input::Down) => self.move_player(0, 1),
            Some(Input::Left) => self.move_player(-1, 0),
            Some(Input::Right) => self.move_player(1, 0),
            _ => false,
        }
    }

    fn move_player(&mut self, dx: i32, dy: i32) -> bool {
        let mut x = 0;
        let mut y = 0;
        let mut moved = false;
        self.world.resource_scope(|world, map: Mut<Map>| {
            world
                .query_filtered::<&mut Position, With<Player>>()
                .for_each_mut(world, |mut position| {
                    x = position.x + dx;
                    y = position.y + dy;
                    if map.is_walkable((x, y)) {
                        position.x = x;
                        position.y = y;
                        moved = true;
                    }
                });
        });
        if moved {
            if let Some(mut player_data) = self.world.get_resource_mut::<PlayerData>() {
                player_data.position = Position::new(x, y);
            }
        }

        moved
    }
}

pub struct PlayerData {
    pub position: Position,
    pub entity: Entity,
}

#[derive(Debug)]
struct Stepper {
    dt: Duration,
    step: Duration,
}

#[allow(dead_code)]
impl Stepper {
    fn new(step: Duration) -> Self {
        Self {
            dt: Duration::new(0, 0),
            step,
        }
    }

    fn advance(&mut self, dt: Duration) -> bool {
        self.dt += dt;

        if self.dt >= self.step {
            loop {
                self.dt -= self.step;
                if self.dt < self.step {
                    break;
                }
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Stepper;

    #[test]
    fn stepper_advance_but_not_enough_returns_false() {
        let mut stepper = Stepper::new(Duration::from_secs(1));

        assert!(!stepper.advance(Duration::from_millis(500)));
    }

    #[test]
    fn stepper_advance_just_enough_returns_true() {
        let mut stepper = Stepper::new(Duration::from_secs(1));

        assert!(stepper.advance(Duration::from_millis(1000)));
    }

    #[test]
    fn stepper_advance_loops_back_after_step() {
        let mut stepper = Stepper::new(Duration::from_secs(1));

        assert!(stepper.advance(Duration::from_millis(1000)));
        assert!(!stepper.advance(Duration::from_millis(999)));
        assert!(stepper.advance(Duration::from_millis(1)));
    }
}
