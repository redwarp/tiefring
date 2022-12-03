use std::{
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};

use bevy_ecs::{
    schedule::{IntoSystemDescriptor, Schedule, SystemStage},
    world::World,
};

use crate::{systems, Shared};

pub struct GameLoop {
    interval: Duration,
    schedule: Arc<Mutex<Schedule>>,
    keep_running: Arc<AtomicBool>,
}

impl Default for GameLoop {
    fn default() -> Self {
        let mut schedule = Schedule::default();
        schedule.add_stage(
            "particles",
            SystemStage::parallel()
                .with_system(systems::spawn_particles)
                .with_system(systems::update_particles.after(systems::spawn_particles)),
        );

        Self {
            interval: Duration::from_micros(16_667),
            schedule: Arc::new(Mutex::new(schedule)),
            keep_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl GameLoop {
    pub fn run(&self, world: Arc<Mutex<World>>) -> std::thread::JoinHandle<()> {
        self.keep_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let keep_running = self.keep_running.clone();
        let interval = self.interval;
        let schedule = self.schedule.clone();

        std::thread::spawn(move || loop {
            let start = Instant::now();
            if !keep_running.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            {
                let mut world = world.lock().unwrap();

                let mut shared = world.resource_mut::<Shared>();
                shared.elapsed_between_redraw = shared.last_update.elapsed();
                shared.last_update = Instant::now();

                schedule.lock().unwrap().run_once(&mut world);
            }

            if !keep_running.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let elapsed = start.elapsed();
            if elapsed < interval {
                std::thread::sleep(interval - elapsed);
            }
        })
    }

    pub fn stop(&mut self) {
        self.keep_running
            .store(false, std::sync::atomic::Ordering::Relaxed)
    }
}
