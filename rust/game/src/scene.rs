use super::*;
use bevy::{
    asset::RenderAssetUsages,
    image::{
        CompressedImageFormats, ImageAddressMode, ImageSampler, ImageSamplerDescriptor, ImageType,
    },
    render::render_resource::{PrimitiveTopology, TextureFormat},
};
use hardtime_core::model::Model;
const SCALE: f32 = 0.02;

pub(super) fn setup(
    mut commands: Commands,
    source: Res<Source>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<TextureCache>,
) {
    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Msaa::Off,
        Transform::from_xyz(0.0, 1.2, 4.5).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
    ));
    let logo = source.root.as_ref().and_then(|root| {
        load_image(
            root,
            Path::new("Graphics/Logo03.png"),
            Some([255, 0, 255]),
            &mut images,
            &mut cache,
        )
        .ok()
    });
    commands.spawn((Panel::Menu,Node { width:percent(100),height:percent(100),align_items:AlignItems::Center,
        padding:UiRect::all(px(40)),..default() },BackgroundColor(Color::srgba(0.01,0.015,0.02,0.72))))
        .with_children(|parent| {
            parent.spawn((Node { width:px(520),padding:UiRect::all(px(32)),flex_direction:FlexDirection::Column,
                row_gap:px(16),border:UiRect::all(px(1)),border_radius:BorderRadius::all(px(8)),..default() },
                BackgroundColor(Color::srgb(0.045,0.055,0.065)),BorderColor::all(Color::srgb(0.23,0.25,0.27))))
                .with_children(|panel| {
                    panel.spawn(text("RUST MIGRATION  /  PREVIEW 01",15.0,ORANGE));
                    if let Some(logo)=logo { panel.spawn((ImageNode::new(logo),Node { width:px(350),height:px(66),..default() })); }
                    else { panel.spawn(text("HARD TIME",42.0,Color::WHITE)); }
                    panel.spawn(text("Back inside Southtown.",25.0,Color::WHITE));
                    panel.spawn(text("Explore the original prison in the new Rust renderer. Gameplay systems are still being ported.",17.0,Color::srgb(0.67,0.71,0.74)));
                    for (label,action) in [("Explore prison  [Enter]",Action::Explore),("Next location  [Tab]",Action::NextRoom),
                        ("Inspect saved games  [S]",Action::SavedGames),("Exit",Action::Quit)] {
                        panel.spawn((Button,Hovered::default(),Node { width:percent(100),height:px(42),
                            align_items:AlignItems::Center,padding:UiRect::horizontal(px(16)),border_radius:BorderRadius::all(px(4)),..default() },
                            BackgroundColor(Color::srgb(0.12,0.15,0.18))))
                            .with_children(|button| { button.spawn(text(label,18.0,Color::WHITE)); })
                            .observe(move |_:On<Activate>,mut navigation:ResMut<Navigation>| { navigation.0=Some(action); });
                    }
                    panel.spawn((Label::Status,text("",15.0,Color::srgb(0.75,0.79,0.82))));
                    panel.spawn(text("Hard Time (c) Mat Dickie 2007\nUnofficial, noncommercial compatibility project.",13.0,Color::srgb(0.47,0.52,0.56)));
                });
        });
    commands
        .spawn((
            Panel::Hud,
            Node {
                position_type: PositionType::Absolute,
                left: px(18),
                top: px(18),
                padding: UiRect::all(px(16)),
                max_width: percent(90),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.025, 0.03, 0.87)),
        ))
        .with_children(|parent| {
            parent.spawn((Label::Hud, text("", 17.0, Color::WHITE)));
        });
}

fn load_image(
    root: &AssetRoot,
    path: &Path,
    mask: Option<[u8; 3]>,
    images: &mut Assets<Image>,
    cache: &mut TextureCache,
) -> Result<Handle<Image>, Box<dyn Error>> {
    let resolved = root.resolve(path)?;
    let key = (resolved.clone(), mask);
    if let Some(image) = cache.0.get(&key) {
        return Ok(image.clone());
    }
    let bytes = root.read(path, 32 * 1024 * 1024)?;
    let extension = resolved
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    let mut image = Image::from_buffer(
        &bytes,
        ImageType::Extension(&extension),
        CompressedImageFormats::NONE,
        true,
        sampler,
        RenderAssetUsages::default(),
    )?;
    if let Some(mask) = mask {
        image = image
            .convert(TextureFormat::Rgba8UnormSrgb)
            .ok_or("Cannot convert menu image")?;
        if let Some(bytes) = &mut image.data {
            for pixel in bytes.as_chunks_mut::<4>().0 {
                if pixel[..3] == mask {
                    pixel[3] = 0;
                }
            }
        }
    }
    let handle = images.add(image);
    cache.0.insert(key, handle.clone());
    Ok(handle)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn load_room(
    mut commands: Commands,
    source: Res<Source>,
    mut session: ResMut<Session>,
    old: Query<Entity, With<WorldObject>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<TextureCache>,
) {
    if session.room_loaded == Some(session.room) {
        return;
    }
    let Some(root) = &source.root else {
        return;
    };
    let room = ROOMS[session.room].1;
    let path = PathBuf::from(format!("World/{room}/{room}.3ds"));
    let model = match root
        .read(&path, 64 * 1024 * 1024)
        .and_then(|bytes| Model::decode(&bytes).map_err(Into::into))
    {
        Ok(model) => model,
        Err(error) => {
            session.message = format!("Cannot load {room}: {error}");
            session.room_error = Some(session.message.clone());
            session.menu = true;
            session.room_loaded = Some(session.room);
            return;
        }
    };
    for entity in &old {
        commands.entity(entity).despawn();
    }
    session.room_error = None;
    let mut material_handles = HashMap::new();
    let mut missing = 0;
    for (name, source_material) in &model.materials {
        let mut masked = false;
        let texture = source_material.texture.as_ref().and_then(|filename| {
            let basename = filename.rsplit(['\\', '/']).next()?;
            // These authored placeholders are replaced by shared masked sprites
            // in Values.bb / World.bb / Crimes.bb. Use the first crowd frame.
            let replacement = match basename.to_ascii_uppercase().as_str() {
                "FENCE.PNG" => Some("Fence.png"),
                "SHOWER.PNG" => Some("Shower.png"),
                "CROWD01.BMP" => Some("Crowd.png"),
                _ => None,
            };
            masked = replacement.is_some();
            let texture_path = if let Some(file) = replacement {
                Path::new("World/Sprites").join(file)
            } else {
                path.parent()?.join(basename)
            };
            match load_image(
                root,
                &texture_path,
                masked.then_some([0, 0, 0]),
                &mut images,
                &mut cache,
            ) {
                Ok(texture) => Some(texture),
                Err(error) => {
                    warn!("{}: {error}", texture_path.display());
                    missing += 1;
                    None
                }
            }
        });
        let color = if texture.is_some() {
            Color::WHITE
        } else {
            Color::srgb_from_array(source_material.diffuse)
        };
        material_handles.insert(
            name.clone(),
            materials.add(StandardMaterial {
                base_color: color.with_alpha(source_material.opacity),
                base_color_texture: texture,
                unlit: true,
                cull_mode: None,
                alpha_mode: if masked {
                    AlphaMode::Mask(0.5)
                } else if source_material.opacity < 1.0 {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Opaque
                },
                ..default()
            }),
        );
    }
    let fallback = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.57, 0.59),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    for object in &model.objects {
        let mut groups = object.materials.clone();
        let mut assigned = vec![false; object.faces.len()];
        for face in groups.values().flatten() {
            assigned[usize::from(*face)] = true;
        }
        let unassigned = assigned
            .iter()
            .enumerate()
            .filter_map(|(i, assigned)| (!assigned).then_some(i as u16))
            .collect::<Vec<_>>();
        if !unassigned.is_empty() {
            groups.insert(String::new(), unassigned);
        }
        for (material_name, faces) in groups {
            if faces.is_empty() {
                continue;
            }
            let material = model.materials.get(&material_name);
            let mut positions = Vec::with_capacity(faces.len() * 3);
            let mut uvs = Vec::with_capacity(faces.len() * 3);
            for face in faces {
                for index in object.faces[usize::from(face)] {
                    let [x, y, z] = object.vertices[usize::from(index)];
                    positions.push([x * SCALE, z * SCALE, -y * SCALE]);
                    let [u, v] = object
                        .uv
                        .get(usize::from(index))
                        .copied()
                        .unwrap_or([0.0; 2]);
                    let (scale, offset) = material
                        .map(|m| (m.uv_scale, m.uv_offset))
                        .unwrap_or(([1.0; 2], [0.0; 2]));
                    let crowd = material
                        .and_then(|m| m.texture.as_deref())
                        .is_some_and(|name| {
                            name.rsplit(['\\', '/'])
                                .next()
                                .is_some_and(|name| name.eq_ignore_ascii_case("CROWD01.BMP"))
                        });
                    uvs.push([
                        u * scale[0] + offset[0],
                        (1.0 - (v * scale[1] + offset[1])) * if crowd { 0.25 } else { 1.0 },
                    ]);
                }
            }
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.compute_flat_normals();
            commands.spawn((
                WorldObject,
                Name::new(object.name.clone()),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(
                    material_handles
                        .get(&material_name)
                        .unwrap_or(&fallback)
                        .clone(),
                ),
            ));
        }
    }
    let (x, z) = match room {
        "Yard" => (8.5, -8.5),
        "Study" | "Hospital" => (1.0, 2.0),
        "Workshop" => (1.0, 1.5),
        "Toilets" => (1.0, 1.0),
        "Courtroom" => (-1.0, 3.0),
        _ => (0.0, 4.5),
    };
    session.position = Vec3::new(x, if room == "Yard" { 1.6 } else { 1.2 }, z);
    session.previous = session.position;
    session.yaw = if room == "Yard" { 2.7 } else { 0.0 };
    session.pitch = if room == "Yard" { -0.1 } else { 0.0 };
    session.room_loaded = Some(session.room);
    session.message = format!(
        "{} | {} objects | {} triangles{}",
        ROOMS[session.room].0,
        model.objects.len(),
        model.triangle_count(),
        if missing > 0 {
            format!(" | {missing} missing textures")
        } else {
            String::new()
        }
    );
    info!("{}", session.message);
}
