use std::{
    borrow::Cow,
    collections::VecDeque,
    f32::consts::TAU,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy_ecs::{
    schedule::{Schedule, SystemStage},
    system::Resource,
    world::World,
};
use loops::GameLoop;
use pollster::FutureExt;
use rand::{rngs::StdRng, SeedableRng};
use systems::{ParticleLifetime, Position, SpawnCommand};
use tiefring::{Canvas, CanvasSettings, Color, SizeInPx};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

mod loops;
mod systems;

#[derive(Resource)]
pub struct Shared {
    pub size: SizeInPx,
    pub last_update: Instant,
    pub elapsed_between_redraw: Duration,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            size: SizeInPx {
                width: 0,
                height: 0,
            },
            last_update: Instant::now(),
            elapsed_between_redraw: Default::default(),
        }
    }
}

impl Shared {
    fn new(width: u32, height: u32) -> Self {
        Shared {
            size: SizeInPx::new(width, height),
            last_update: Instant::now(),
            elapsed_between_redraw: Default::default(),
        }
    }
}

#[derive(Resource)]
pub struct Random(StdRng);

impl Random {
    fn new() -> Self {
        Self(StdRng::seed_from_u64(42))
    }
}

impl Deref for Random {
    type Target = StdRng;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Random {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

struct FPSCounter {
    ticks: VecDeque<Instant>,
}

impl FPSCounter {
    fn new() -> Self {
        Self {
            ticks: VecDeque::with_capacity(256),
        }
    }

    fn tick(&mut self) -> usize {
        let now = Instant::now();
        let some_time_ago = now - Duration::from_secs(1);
        while self
            .ticks
            .front()
            .map_or(false, |tick| *tick < some_time_ago)
        {
            self.ticks.pop_front();
        }
        self.ticks.push_back(now);

        self.ticks.len()
    }
}

fn main() {
    let event_loop = EventLoop::new();
    let mut input_helper = WinitInputHelper::new();

    let shared = Shared::new(600, 400);

    let window = {
        let size = LogicalSize::new(shared.size.width, shared.size.height);
        WindowBuilder::new()
            .with_title("Particles")
            .with_inner_size(size)
            .with_resizable(true)
            .with_visible(false)
            .build(&event_loop)
            .unwrap()
    };

    let mut canvas = Canvas::new(
        &window,
        shared.size.width,
        shared.size.height,
        CanvasSettings::default(),
    )
    .block_on()
    .unwrap();

    window.set_visible(true);

    let sprites = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("particles/sprites")
        .unwrap();

    let resources = canvas.resources();

    let star = resources.load_sprite(sprites.join("star.png")).unwrap();

    let fonts = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("resources/fonts")
        .unwrap();

    let mut roboto_regular = resources
        .load_font(fonts.join("Roboto-Regular.ttf"))
        .unwrap();

    let time = Shared::default();
    let mut world = World::new();
    world.insert_resource(time);
    world.insert_resource(Random::new());
    let world = Arc::new(Mutex::new(world));

    let mut schedule = Schedule::default();
    schedule.add_stage(
        "spawn",
        SystemStage::parallel().with_system(systems::spawn_particles),
    );
    schedule.add_stage_after(
        "spawn",
        "animate",
        SystemStage::parallel().with_system(systems::update_particles),
    );

    let mut game_loop = GameLoop::default();
    let mut handle = Some(game_loop.run(world.clone()));

    let mut fps_counter = FPSCounter::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;

                    game_loop.stop();
                    if let Some(handle) = handle.take() {
                        handle.join().unwrap();
                    }
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                canvas
                    .draw(|graphics| {
                        let mut world = world.lock().unwrap();
                        let mut query = world.query::<(&Position, &ParticleLifetime)>();
                        let mut count: usize = 0;
                        for (position, particle_lifetime) in query.iter(&world) {
                            count += 1;
                            graphics
                                .draw_sprite(&star, (position.x, position.y))
                                .rotate(TAU * particle_lifetime.freshness())
                                .alpha(particle_lifetime.freshness());
                        }

                        let fps = fps_counter.tick();
                        let text: Cow<_> = if count == 0 {
                            "Press + to spawn stars".into()
                        } else {
                            format!("Showing {count} stars at {fps} FPS").into()
                        };
                        graphics.draw_text(
                            &mut roboto_regular,
                            &text,
                            20,
                            tiefring::Position::new(0.0, 0.0),
                            Color::rgb(1.0, 1.0, 1.0),
                        );
                    })
                    .unwrap();
            }
            _ => {}
        }

        if input_helper.update(&event) {
            if input_helper.key_held(VirtualKeyCode::NumpadAdd)
                || input_helper.key_pressed(VirtualKeyCode::NumpadAdd)
            {
                for _ in 0..200 {
                    world.lock().unwrap().spawn(SpawnCommand);
                }
            }

            if let Some(size) = input_helper.window_resized() {
                canvas.set_size(size.width, size.height);
                world.lock().unwrap().resource_mut::<Shared>().size =
                    SizeInPx::new(size.width, size.height);
            }

            window.request_redraw();
        }
    });
}
