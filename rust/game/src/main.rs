//! Rust migration preview of Hard Time. Original game copyright Mat Dickie 2007.
//! Owns its renderer; does not launch or link Blitz3D.
mod scene;
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    picking::hover::Hovered,
    prelude::*,
    render::{
        RenderPlugin,
        settings::{Backends, Dx12Compiler, RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
    text::FontSize,
    ui_widgets::{Activate, Button},
    window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow},
};
use hardtime_core::{
    assets::AssetRoot,
    clock::FixedClock,
    legacy::{Options, Progress},
};
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const ROOMS: [(&str, &str); 9] = [
    ("Cell block", "Block"),
    ("Exercise yard", "Yard"),
    ("Main hall", "Hall"),
    ("Study", "Study"),
    ("Hospital", "Hospital"),
    ("Kitchen", "Kitchen"),
    ("Workshop", "Workshop"),
    ("Toilets", "Toilets"),
    ("Courtroom", "Courtroom"),
];
const ORANGE: Color = Color::srgb(0.95, 0.49, 0.25);
#[derive(Resource)]
struct Source {
    root: Option<AssetRoot>,
    slots: String,
}
#[derive(Resource, Default)]
struct TextureCache(HashMap<(PathBuf, Option<[u8; 3]>), Handle<Image>>);
#[derive(Resource)]
struct Session {
    menu: bool,
    paused: bool,
    room: usize,
    room_loaded: Option<usize>,
    room_error: Option<String>,
    message: String,
    position: Vec3,
    previous: Vec3,
    yaw: f32,
    pitch: f32,
    clock: FixedClock,
    frames: u64,
    stats_start: f64,
    stats_frames: u64,
    stats_ticks: u64,
    fps: f64,
    tick_rate: f64,
    screenshot_count: u32,
    show_stats: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            menu: true,
            paused: false,
            room: 0,
            room_loaded: None,
            room_error: None,
            message: String::new(),
            position: Vec3::new(0.0, 1.2, 4.5),
            previous: Vec3::new(0.0, 1.2, 4.5),
            yaw: 0.0,
            pitch: 0.0,
            clock: FixedClock::default(),
            frames: 0,
            stats_start: 0.0,
            stats_frames: 0,
            stats_ticks: 0,
            fps: 0.0,
            tick_rate: 0.0,
            screenshot_count: 0,
            show_stats: true,
        }
    }
}
#[derive(Clone, Copy)]
enum Action {
    Explore,
    NextRoom,
    SavedGames,
    Quit,
}
#[derive(Resource, Default)]
struct Navigation(Option<Action>);
#[derive(Component)]
struct WorldObject;
#[derive(Component)]
struct GameCamera;
#[derive(Component)]
enum Panel {
    Menu,
    Hud,
}
#[derive(Component)]
enum Label {
    Status,
    Hud,
}
#[derive(Resource)]
struct CaptureOutput(PathBuf);
#[derive(Resource, Default)]
struct Smoke {
    enabled: bool,
    stage: u32,
    completed: u32,
    deadline: f64,
    moved_ticks: u32,
}

fn main() {
    if let Err(error) = start() {
        eprintln!("Hard Time Rust: {error}");
        std::process::exit(1);
    }
}
fn start() -> Result<(), Box<dyn Error>> {
    let mut assets = None;
    let mut software = false;
    let mut smoke = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--assets") => {
                assets = Some(PathBuf::from(args.next().ok_or("--assets needs a folder")?))
            }
            Some("--software") => software = true,
            Some("--smoke-test") => {
                smoke = Some(PathBuf::from(
                    args.next()
                        .ok_or("--smoke-test needs a new output folder")?,
                ))
            }
            Some("--help" | "-h") => {
                println!(
                    "Hard Time Rust migration preview\n  --assets <game folder>\n  --software (Direct3D software adapter)\n  --smoke-test <new evidence folder>\n\nEnter: explore; Tab: next room; WASD/arrows: move; Q/E: down/up; right mouse: look; P: pause; Esc: menu; F11: stats; F12: screenshot.\nOriginal assets and saves are read only. Combat, AI, missions, dialogue and character animation are not ported yet."
                );
                return Ok(());
            }
            _ if assets.is_none() && Path::new(&arg).is_dir() => assets = Some(PathBuf::from(arg)),
            _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy()).into()),
        }
    }
    let root = if let Some(path) = assets {
        Some(AssetRoot::open(path)?)
    } else {
        find_assets()
    };
    let slots=root.as_ref().map(saved_slots).unwrap_or_else(|| "No game folder found. Drop a game folder on this executable, or pass --assets <folder>.".to_owned());
    if smoke.is_some() && root.is_none() {
        return Err("The smoke test requires an original game folder (--assets <folder>)".into());
    }
    let session = Session {
        message: if root.is_none() {
            slots.clone()
        } else {
            String::new()
        },
        ..default()
    };
    let output = smoke.clone().unwrap_or_else(|| {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Hard Time Rust")
            .join("Screenshots")
    });
    if smoke.is_some() {
        std::fs::create_dir(&output)?;
    }
    let mut settings = WgpuSettings {
        force_fallback_adapter: software,
        ..default()
    };
    if cfg!(target_os = "windows") {
        settings.backends = Some(Backends::DX12);
        // FXC ships with the supported Windows versions; no DXC DLL download.
        settings.dx12_shader_compiler = Dx12Compiler::Fxc;
    }
    let exit = App::new()
        .insert_resource(Source { root, slots })
        .insert_resource(session)
        .insert_resource(TextureCache::default())
        .insert_resource(Navigation::default())
        .insert_resource(CaptureOutput(output))
        .insert_resource(Smoke {
            enabled: smoke.is_some(),
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.025, 0.032, 0.04)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hard Time — Rust preview".into(),
                        resolution: (1280, 720).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(settings)),
                    ..default()
                }),
        )
        .add_systems(Startup, scene::setup)
        .add_systems(
            Update,
            (input, scene::load_room, movement, interface, smoke_test).chain(),
        )
        .run();
    if exit.is_error() {
        return Err("The preview exited with an error; see the diagnostic output above".into());
    }
    Ok(())
}
fn find_assets() -> Option<AssetRoot> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        paths.push(parent.to_owned());
    }
    if let Ok(current) = std::env::current_dir() {
        paths.push(current.clone());
        paths.push(current.join("dist/Hard-Time-Windows-x64"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join("Programs/Hard Time"));
    }
    paths
        .into_iter()
        .find_map(|path| AssetRoot::open(path).ok())
}
fn saved_slots(root: &AssetRoot) -> String {
    let result = (|| -> Result<String, Box<dyn Error>> {
        let options = Options::decode(&root.read("Data/Options.dat", 1024)?)?;
        let mut lines = vec!["Imported save slots (read only)".to_owned()];
        for slot in 1..=3 {
            let path = format!("Data/Slot{slot:02}/Progress.dat");
            if root.resolve(&path).is_err() {
                lines.push(format!("{slot}. Empty"));
                continue;
            }
            match Progress::decode(&root.read(path, 4096)?) {
                Ok(p) => lines.push(format!(
                    "{slot}. {}  |  ${}  |  {}:{:02}  |  {} characters",
                    String::from_utf8_lossy(&options.slot_names[slot - 1]),
                    p.status[4],
                    p.time[3],
                    p.time[2],
                    p.status[0]
                )),
                Err(error) => lines.push(format!("{slot}. Cannot read this save: {error}")),
            }
        }
        Ok(lines.join("\n"))
    })();
    result.unwrap_or_else(|error| format!("Save import: {error}"))
}
fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

#[allow(clippy::too_many_arguments)]
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut session: ResMut<Session>,
    mut navigation: ResMut<Navigation>,
    source: Res<Source>,
    mut commands: Commands,
    output: Res<CaptureOutput>,
    smoke: Res<Smoke>,
    mut window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Enter) && session.menu {
        navigation.0 = Some(Action::Explore);
    }
    if keys.just_pressed(KeyCode::Tab) {
        navigation.0 = Some(Action::NextRoom);
    }
    if keys.just_pressed(KeyCode::KeyS) && session.menu {
        navigation.0 = Some(Action::SavedGames);
    }
    if keys.just_pressed(KeyCode::Escape) {
        session.menu = !session.menu;
        session.paused = false;
    }
    if keys.just_pressed(KeyCode::KeyP) && !session.menu {
        session.paused = !session.paused;
    }
    if keys.just_pressed(KeyCode::F11) {
        session.show_stats = !session.show_stats;
    }
    if keys.just_pressed(KeyCode::F12) {
        session.screenshot_count += 1;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        match std::fs::create_dir_all(&output.0) {
            Ok(()) => {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(output.0.join(format!(
                        "Hard-Time-{stamp}-{}.png",
                        session.screenshot_count
                    ))));
            }
            Err(error) => session.message = format!("Cannot save screenshot: {error}"),
        }
    }
    match navigation.0.take() {
        Some(Action::Explore) if source.root.is_some() => {
            session.menu = false;
            session.paused = false;
        }
        Some(Action::Explore) => {
            session.message = "No assets found. Start with --assets <your game folder>.".into()
        }
        Some(Action::NextRoom) => session.room = (session.room + 1) % ROOMS.len(),
        Some(Action::SavedGames) => {
            session.menu = true;
            session.message = source.slots.clone();
        }
        Some(Action::Quit) => {
            commands.write_message(AppExit::Success);
        }
        None => {}
    }
    if !window.0.focused && !session.menu && !smoke.enabled {
        session.paused = true;
    }
    let looking =
        mouse.pressed(MouseButton::Right) && !session.menu && !session.paused && window.0.focused;
    window.1.grab_mode = if looking {
        CursorGrabMode::Confined
    } else {
        CursorGrabMode::None
    };
    window.1.visible = !looking;
    if looking {
        session.yaw -= motion.delta.x * 0.003;
        session.pitch = (session.pitch - motion.delta.y * 0.003).clamp(-1.5, 1.5);
    }
}
fn movement(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut cameras: Query<&mut Transform, With<GameCamera>>,
    mut smoke: ResMut<Smoke>,
) {
    let paused = session.menu || session.paused;
    let step = session.clock.advance(time.delta(), paused);
    if paused {
        session.previous = session.position;
    }
    let mut movement = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        movement.z -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        movement.z += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        movement.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        movement.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyQ) {
        movement.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
        movement.y += 1.0;
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        6.0
    } else {
        3.0
    };
    let velocity = Quat::from_rotation_y(session.yaw) * movement.normalize_or_zero() * speed / 30.0;
    for _ in 0..step.ticks {
        session.previous = session.position;
        if smoke.enabled && smoke.stage == 2 && smoke.moved_ticks < 30 {
            session.position.x += 0.025;
            smoke.moved_ticks += 1;
        } else {
            session.position += velocity;
        }
    }
    for mut transform in &mut cameras {
        transform.translation = session.previous.lerp(
            session.position,
            if paused { 1.0 } else { step.interpolation },
        );
        transform.rotation = Quat::from_euler(EulerRot::YXZ, session.yaw, session.pitch, 0.0);
    }
    session.frames += 1;
    let now = time.elapsed_secs_f64();
    if now - session.stats_start >= 1.0 {
        session.fps = (session.frames - session.stats_frames) as f64 / (now - session.stats_start);
        session.tick_rate =
            (session.clock.ticks - session.stats_ticks) as f64 / (now - session.stats_start);
        session.stats_frames = session.frames;
        session.stats_ticks = session.clock.ticks;
        session.stats_start = now;
    }
}
fn interface(
    session: Res<Session>,
    mut panels: Query<(&Panel, &mut Node)>,
    mut labels: Query<(&Label, &mut Text)>,
    mut buttons: Query<(&Hovered, &mut BackgroundColor), With<Button>>,
) {
    for (panel, mut node) in &mut panels {
        node.display = if matches!(panel, Panel::Menu) == session.menu {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (label, mut text) in &mut labels {
        text.0 = match label {
            Label::Status => session.message.clone(),
            Label::Hud => format!(
                "HARD TIME  /  RUST PREVIEW\n{}{}\nWASD / arrows: fly   Q / E: down / up   Right mouse: look\nTab: next location   P: pause   Esc: menu   F12: screenshot\n{}",
                ROOMS[session.room].0,
                if session.paused {
                    "  -  PAUSED (P to resume)"
                } else {
                    ""
                },
                if session.show_stats {
                    format!(
                        "{:.0} FPS | {:.0} ticks/s | {} total ticks",
                        session.fps, session.tick_rate, session.clock.ticks
                    )
                } else {
                    String::new()
                }
            ),
        };
    }
    for (hovered, mut background) in &mut buttons {
        background.0 = if hovered.get() {
            Color::srgb(0.31, 0.19, 0.13)
        } else {
            Color::srgb(0.12, 0.15, 0.18)
        };
    }
}
fn smoke_test(
    mut commands: Commands,
    mut smoke: ResMut<Smoke>,
    time: Res<Time<Real>>,
    session: Res<Session>,
    mut navigation: ResMut<Navigation>,
    output: Res<CaptureOutput>,
) {
    if !smoke.enabled {
        return;
    }
    if let Some(error) = &session.room_error {
        error!("Rust smoke test: {error}");
        commands.write_message(AppExit::error());
        smoke.enabled = false;
        return;
    }
    let now = time.elapsed_secs_f64();
    let capture = match smoke.stage {
        0 if now > 5.0 && session.frames >= 30 => Some("menu.png"),
        1 if smoke.completed >= 1 => {
            navigation.0 = Some(Action::Explore);
            smoke.stage = 2;
            smoke.deadline = now + 3.0;
            None
        }
        2 if now > smoke.deadline && smoke.moved_ticks == 30 => Some("prison.png"),
        3 if smoke.completed >= 2 && now > smoke.deadline => Some("prison-after-capture.png"),
        4 if smoke.completed >= 3 => {
            navigation.0 = Some(Action::NextRoom);
            smoke.stage = 5;
            smoke.deadline = now + 3.0;
            None
        }
        5 if now > smoke.deadline => Some("yard.png"),
        6 if smoke.completed >= 4 => {
            navigation.0 = Some(Action::NextRoom);
            smoke.stage = 7;
            smoke.deadline = now + 2.0;
            None
        }
        7 if now > smoke.deadline => Some(match session.room {
            2 => "hall.png",
            3 => "study.png",
            4 => "hospital.png",
            5 => "kitchen.png",
            6 => "workshop.png",
            7 => "toilets.png",
            8 => "courtroom.png",
            _ => unreachable!(),
        }),
        8 if smoke.completed >= session.room as u32 + 3 => {
            if session.room < ROOMS.len() - 1 {
                navigation.0 = Some(Action::NextRoom);
                smoke.stage = 7;
                smoke.deadline = now + 2.0;
                return;
            }
            navigation.0 = Some(Action::SavedGames);
            smoke.stage = 9;
            smoke.deadline = now + 1.0;
            None
        }
        9 if now > smoke.deadline => Some("saved-games.png"),
        10 if smoke.completed >= 12 => {
            let result = format!(
                "PASS Rust window, all nine original rooms loaded, 30 movement ticks, save-slot menu, twelve screenshot capture events, clean exit\nInspect PNG files separately to verify rendered output.\nframes={}\nticks={}\n",
                session.frames, session.clock.ticks
            );
            match std::fs::write(output.0.join("result.txt"), result) {
                Ok(()) => {
                    commands.write_message(AppExit::Success);
                }
                Err(error) => {
                    error!("Smoke result: {error}");
                    commands.write_message(AppExit::error());
                }
            }
            smoke.enabled = false;
            None
        }
        _ => None,
    };
    if let Some(name) = capture {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(output.0.join(name)))
            .observe(|_: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
                smoke.completed += 1;
            });
        smoke.stage += 1;
        smoke.deadline = now + 1.0;
    }
    if now > 120.0 {
        error!("Rust smoke test timed out");
        commands.write_message(AppExit::error());
        smoke.enabled = false;
    }
}
