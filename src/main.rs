use bevy::{
    render::view::VisibleEntities,
    sprite::Mesh2dHandle,
    core_pipeline::{
        bloom::{BloomCompositeMode, BloomSettings},
        tonemapping::Tonemapping,
        },
    //tasks::IoTaskPool,
    prelude::*};

//use std::{fs::File, io::Write};

use bevy_pancam::{PanCam, PanCamPlugin};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
//use rand::prelude::random;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: String::from("awawawa"),
                ..default()
            }),
            ..default()
        }))
        //RESOURCES
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(Msaa::Sample4)
        //PLUGINS
        .add_plugins(PanCamPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        //SYSTEMS
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_pan.run_if(in_state(Mode::Edit)))
        .add_systems(Update, save_scene)

        .insert_resource(CursorInfo::default())
        .add_systems(Update, update_cursor_info)

        .add_state::<Mode>()

        .register_type::<Radius>()

        .register_type::<Selected>()
        .register_type::<Visible>()

        .register_type::<Depth>()
        .register_type::<Order>()
        .register_type::<Num>()
        .register_type::<Arr>()
        .register_type::<Offset>()

        // test high depth
        .insert_resource(Depth(-10.))

        .add_systems(Update, spawn_circles.run_if(in_state(Mode::Draw)))
        .add_systems(Update, draw_pointer_circle.run_if(not(in_state(Mode::Connect))))
        .add_systems(Update, mark_visible.after(update_cursor_info))
        .add_systems(Update, update_selection.after(mark_visible).run_if(in_state(Mode::Edit)))
        .add_systems(Update, move_selected.after(update_selection).run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_color.after(update_selection).run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_radius.after(update_selection).run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_num.after(update_selection).run_if(in_state(Mode::Edit)))
        .add_systems(Update, highlight_selected.run_if(in_state(Mode::Edit)))
        .add_systems(Update, delete_selected.run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_order.run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_order_text.run_if(in_state(Mode::Edit)))
        .add_systems(Update, switch_mode)

        .add_systems(Update, connect.run_if(in_state(Mode::Connect)))
        .add_systems(Update, draw_connections)
        .add_systems(Update, draw_connecting_line.run_if(in_state(Mode::Connect)))
        .add_systems(Update, update_link_type.run_if(in_state(Mode::Edit)))
        .add_systems(Update, update_link_type_text.run_if(in_state(Mode::Edit)))

        .insert_resource(BloomCircleId(Entity::from_raw(0)))
        .add_systems(Startup, spawn_bloom_circle)
        .add_systems(Update, update_bloom_settings)
        .add_systems(Update, update_color_from_input)
        .add_systems(Update, update_num_from_input)
        .run();
}

fn setup(
    mut commands: Commands,
    mut config: ResMut<GizmoConfig>,
) {
    config.line_width = 1.;
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_translation(Vec3::Z), //push the camera "back" one unit
        ..default()
        },
        BloomSettings::default(), //enable bloom
        PanCam {
            enabled: false,
            //limit zooming
            max_scale: Some(80.),
            min_scale: 0.005,
            ..default()
        },
    ));
}

fn toggle_pan(
    mut query: Query<&mut PanCam>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        let mut pancam = query.single_mut();
        pancam.enabled = true;
    }
    if keyboard_input.just_released(KeyCode::Space) {
        let mut pancam = query.single_mut();
        pancam.enabled = false;
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum Mode {
    #[default]
    Draw,
    Connect,
    Edit,
}

fn switch_mode(
    mut next_state: ResMut<NextState<Mode>>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        if keyboard_input.just_pressed(KeyCode::Key1) { next_state.set(Mode::Draw); }
        if keyboard_input.just_pressed(KeyCode::Key2) { next_state.set(Mode::Connect); }
        if keyboard_input.just_pressed(KeyCode::Key3) { next_state.set(Mode::Edit); }
    }
}

// own file format?
// query the info needed to respawn the same entities on load
// switching?
// creating multiple worlds, switching between them, and saving/loading them
fn save_scene(
    circles_query: Query<&GlobalTransform, With<Order>>,
    keyboard_input: Res<Input<KeyCode>>,
    ) {
    let ctrl = keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keyboard_input.just_pressed(KeyCode::S) {
        for circle in circles_query.iter() {
            info!("{:?}", circle.translation());
        }
    }
//        #[cfg(not(target_arch = "wasm32"))]
//        IoTaskPool::get()
//            .spawn(async move {
//                File::create(format!("scene"))
//                    .and_then(|mut file| file.write(serialized_scene.as_bytes()))
//                    .expect("Error while writing scene to file");
//            })
//            .detach();
//    }
}

// ---------------------- cursor ------------------------

// initial, final, delta
#[derive(Resource, Default)]
struct CursorInfo {
    i: Vec2,
    f: Vec2,
    d: Vec2,
}

fn update_cursor_info(
    mouse_button_input: Res<Input<MouseButton>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mut cursor: ResMut<CursorInfo>,
    mut last_pos: Local<Vec2>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let (cam, cam_transform) = camera_query.single();
        if let Some(cursor_pos) = windows.single().cursor_position() {
            if let Some(point) = cam.viewport_to_world_2d(cam_transform, cursor_pos) {
                cursor.i = point;
            }
        }
    }
    if mouse_button_input.pressed(MouseButton::Left) {
        let (cam, cam_transform) = camera_query.single();
        if let Some(cursor_pos) = windows.single().cursor_position() {
            if let Some(point) = cam.viewport_to_world_2d(cam_transform, cursor_pos) {
                cursor.f = point;
                cursor.d = point - *last_pos;
                *last_pos = point;
            }
        }
    }
    if mouse_button_input.just_released(MouseButton::Left) {
        cursor.d = Vec2::ZERO;
        *last_pos = -cursor.f; // so on the pressed frame we don't get a delta
    }
}

// ------------- circles ------------------------

#[derive(Component, Reflect)]
struct Num(f32);

#[derive(Component, Reflect)]
struct Arr(Vec<f32>);

#[derive(Component, Reflect)]
struct Offset {
    trans: Vec3,
    color: Color,
    radius: f32,
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Depth(f32);

#[derive(Component, Reflect)]
struct Radius(f32);

#[derive(Component, Reflect)]
struct Selected;

#[derive(Component, Reflect)]
struct Visible;

#[derive(Component, Reflect)]
struct Order(usize);

fn spawn_circles(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut depth: ResMut<Depth>,
    cursor: Res<CursorInfo>,
) {
    if mouse_button_input.just_released(MouseButton::Left) {
        let radius = cursor.f.distance(cursor.i);
        let id = commands.spawn((
            ColorMesh2dBundle {
                mesh: meshes.add(shape::Circle::new(radius).into()).into(),
                material: materials.add(ColorMaterial::from(Color::hsl(0., 1.0, 0.5))),
                transform: Transform::from_translation(cursor.i.extend(depth.0)),
                ..default()
            },
            Radius(radius),
            Visible, //otherwise it can't be selected til after mark_visible is updated
            Order(0),
            Num(0.),
            Arr(Vec::new()),
            Offset {trans:Vec3::ZERO, color:Color::BLACK, radius:0.},
        )).id();

        // have the circle adopt a text entity
        let text = commands.spawn(Text2dBundle {
            text: Text::from_sections([
                TextSection::new(
                    "order: ".to_string() + &0.to_string() + "\n",
                    TextStyle::default()
                ),
                TextSection::new(
                    0.to_string(),
                    TextStyle::default()
                ),
            ]),
            transform: Transform::from_translation(Vec3{z:0.000001, ..default()}),
            ..default()
        }).id();
        commands.entity(id).add_child(text);

        depth.0 += 0.00001;
    }
}

fn draw_pointer_circle(
    cursor: Res<CursorInfo>,
    mut gizmos: Gizmos,
    time: Res<Time>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    if mouse_button_input.pressed(MouseButton::Left) {
        let color = Color::hsl((time.elapsed_seconds() * 100.) % 360., 1.0, 0.5);
        gizmos.circle_2d(cursor.i, cursor.f.distance(cursor.i), color).segments(64);
    }
}

fn highlight_selected(
    mut gizmos: Gizmos,
    time: Res<Time>,
    query: Query<(&Radius, &GlobalTransform), With<Selected>>,
) {
    for (r, t) in query.iter() {
        let color = Color::hsl((time.elapsed_seconds() * 100.) % 360., 1.0, 0.5);
        gizmos.circle_2d(t.translation().xy(), r.0, color).segments(64);
    }
}

// loop over the visible entities and give them a Visible component
// so we can query just the visible entities
fn mark_visible(
    mouse_button_input: Res<Input<MouseButton>>,
    mut commands: Commands,
    query: Query<Entity, With<Visible>>,
    visible: Query<&VisibleEntities>,
) {
    if mouse_button_input.just_released(MouseButton::Left) {
        for e in query.iter() {
            commands.entity(e).remove::<Visible>();
        }
        let vis = visible.single();
        for e in vis.iter() {
            commands.entity(*e).insert(Visible);
        }
    }
}

//optimize all those distance calls, use a distance squared instead
fn update_selection(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    query: Query<(Entity, &Radius, &GlobalTransform), Or<(With<Visible>, With<Selected>)>>,
    selected: Query<Entity, With<Selected>>,
    selected_query: Query<&Selected>,
    cursor: Res<CursorInfo>,
    keyboard_input: Res<Input<KeyCode>>,
    mut top_clicked_circle: Local<Option<(Entity, f32)>>,
) {
    let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if mouse_button_input.just_pressed(MouseButton::Left) {
        for (e, r, t) in query.iter() {
            if top_clicked_circle.is_some() {
                if t.translation().z > top_clicked_circle.unwrap().1 &&
                    cursor.i.distance(t.translation().xy()) < r.0 {
                    *top_clicked_circle = Some((e, t.translation().z));
                }
            } else {
                if cursor.i.distance(t.translation().xy()) < r.0 {
                    *top_clicked_circle = Some((e, t.translation().z));
                }
            }
        }
        if let Some(top) = *top_clicked_circle {
            if !selected_query.contains(top.0) {
                if shift { commands.entity(top.0).insert(Selected); }
                else {
                    for entity in selected.iter() {
                        commands.entity(entity).remove::<Selected>();
                    }
                    commands.entity(top.0).insert(Selected);
                }
            }
        }
    }
    if mouse_button_input.just_released(MouseButton::Left) {
        if top_clicked_circle.is_none() {
            if !shift {
                for entity in selected.iter() {
                    commands.entity(entity).remove::<Selected>();
                }
            }
            // select those in the dragged area
            for (e, r, t) in query.iter() {
                if cursor.i.distance(cursor.f) + r.0 > cursor.i.distance(t.translation().xy()) {
                    commands.entity(e).insert(Selected);
                }
            }
        }
        *top_clicked_circle = None;
    }
}

fn move_selected(
    mouse_button_input: Res<Input<MouseButton>>,
    cursor: Res<CursorInfo>,
    mut query: Query<&mut Transform, With<Selected>>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if keyboard_input.pressed(KeyCode::Key1) {
        if mouse_button_input.pressed(MouseButton::Left) &&
        //lol because the update to entities isn't read until the next frame
        !mouse_button_input.just_pressed(MouseButton::Left) {
            for mut t in query.iter_mut() {
                t.translation.x += cursor.d.x;
                t.translation.y += cursor.d.y;
            }
        }
        if keyboard_input.pressed(KeyCode::Up) {
            for mut t in query.iter_mut() { t.translation.y += 1.; }
        }
        if keyboard_input.pressed(KeyCode::Down) {
            for mut t in query.iter_mut() { t.translation.y -= 1.; }
        }
        if keyboard_input.pressed(KeyCode::Right) {
            for mut t in query.iter_mut() { t.translation.x += 1.; }
        }
        if keyboard_input.pressed(KeyCode::Left) {
            for mut t in query.iter_mut() { t.translation.x -= 1.; }
        }
    }
}

fn update_color(
    mut mats: ResMut<Assets<ColorMaterial>>,
    material_ids: Query<(&Handle<ColorMaterial>, &Children), With<Selected>>,
    keyboard_input: Res<Input<KeyCode>>,
    cursor: Res<CursorInfo>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut white_hole_query: Query<&mut WhiteHole>,
    black_hole_query: Query<&BlackHole>,
) {
    if keyboard_input.pressed(KeyCode::Key2) {
        if mouse_button_input.pressed(MouseButton::Left) &&
        !mouse_button_input.just_pressed(MouseButton::Left) {
            for (id, children) in material_ids.iter() {
                let mat = mats.get_mut(id).unwrap();
                mat.color.set_h((mat.color.h() + cursor.d.x).rem_euclid(360.));
                // mark change
                for child in children.iter() {
                    if let Ok(black_hole) = black_hole_query.get(*child) {
                        if black_hole.link_type == -2 {
                            white_hole_query.get_mut(black_hole.wh).unwrap().changed = true;
                        }
                    }
                }
            }
        }

        let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let increment = if shift { 0.01 } else { -0.01 };
        if keyboard_input.pressed(KeyCode::Up) {
            for (id, _) in material_ids.iter() {
                let mat = mats.get_mut(id).unwrap();
                mat.color.set_h((mat.color.h() + increment * 100.).rem_euclid(360.));
            }
        }
        if keyboard_input.pressed(KeyCode::Down) {
            for (id, _) in material_ids.iter() {
                let mat = mats.get_mut(id).unwrap();
                mat.color.set_s((mat.color.s() + increment).rem_euclid(2.));
            }
        }
        if keyboard_input.pressed(KeyCode::Right) {
            for (id, _) in material_ids.iter() {
                let mat = mats.get_mut(id).unwrap();
                mat.color.set_l((mat.color.l() + increment).rem_euclid(4.));
            }
        }
        if keyboard_input.pressed(KeyCode::Left) {
            for (id, _) in material_ids.iter() {
                let mat = mats.get_mut(id).unwrap();
                mat.color.set_a((mat.color.a() + increment).rem_euclid(1.));
            }
        }
    }
}

fn update_radius(
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_ids: Query<(Entity, &Mesh2dHandle), With<Selected>>,
    keyboard_input: Res<Input<KeyCode>>,
    cursor: Res<CursorInfo>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut radius_query: Query<&mut Radius>,
) {
    if keyboard_input.pressed(KeyCode::Key3) {
        if mouse_button_input.pressed(MouseButton::Left) &&
        !mouse_button_input.just_pressed(MouseButton::Left) {
            for (entity, Mesh2dHandle(id)) in mesh_ids.iter() {
                let r = cursor.f.distance(cursor.i);
                let mesh = meshes.get_mut(id).unwrap();
                *mesh = shape::Circle::new(r).into();
                radius_query.get_mut(entity).unwrap().0 = r;
            }
        }
        if keyboard_input.pressed(KeyCode::Up) {
            for (entity, Mesh2dHandle(id)) in mesh_ids.iter() {
                let r = radius_query.get_mut(entity).unwrap().0 + 1.;
                radius_query.get_mut(entity).unwrap().0 = r;
                let mesh = meshes.get_mut(id).unwrap();
                *mesh = shape::Circle::new(r).into();
            }
        }
        if keyboard_input.pressed(KeyCode::Down) {
            for (entity, Mesh2dHandle(id)) in mesh_ids.iter() {
                let r = radius_query.get_mut(entity).unwrap().0 - 1.;
                radius_query.get_mut(entity).unwrap().0 = r;
                let mesh = meshes.get_mut(id).unwrap();
                *mesh = shape::Circle::new(r).into();
            }
        }
    }
}

fn update_num(
    mut query: Query<(&mut Num, &Children), With<Selected>>,
    keyboard_input: Res<Input<KeyCode>>,
    cursor: Res<CursorInfo>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut white_hole_query: Query<&mut WhiteHole>,
    black_hole_query: Query<&BlackHole>,
) {
    if keyboard_input.pressed(KeyCode::Key4) {
        if mouse_button_input.pressed(MouseButton::Left) &&
        !mouse_button_input.just_pressed(MouseButton::Left) {
            for (mut n, children) in query.iter_mut() {
                // change the number
                n.0 += cursor.d.y / 10.;
                // inform any white holes connected through link 4 black holes
                // that our value has changed
                for child in children.iter() {
                    if let Ok(black_hole) = black_hole_query.get(*child) {
                        if black_hole.link_type == -4 {
                            white_hole_query.get_mut(black_hole.wh).unwrap().changed = true;
                        }
                    }
                }
            }
        }
    }
}
            

fn update_order (
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Order, With<Selected>>,
) {
    let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if shift && keyboard_input.just_pressed(KeyCode::Up) {
        for mut order in query.iter_mut() { order.0 += 1; }
    }
    if shift && keyboard_input.just_pressed(KeyCode::Down) {
        for mut order in query.iter_mut() { if order.0 > 0 { order.0 -= 1; } }
    }
}

fn update_order_text(
    mut query: Query<(&mut Text, &Parent), With<Visible>>,
    order_query: Query<&Order>,
    num_query: Query<&Num>,
) {
    for (mut text, parent) in query.iter_mut() {
        if let Ok(order) = order_query.get(**parent) {
            text.sections[0].value = "order: ".to_string() + &order.0.to_string() + "\n";
        }
        if let Ok(num) = num_query.get(**parent) {
            text.sections[1].value = num.0.to_string();
        }
    }
}

fn delete_selected(
    keyboard_input: Res<Input<KeyCode>>,
    query: Query<(Entity, &Children), With<Selected>>,
    mut commands: Commands,
    white_hole_query: Query<&WhiteHole>,
    black_hole_query: Query<&BlackHole>,
) {
    if keyboard_input.pressed(KeyCode::Delete) {
        for (id, children) in query.iter() {
            // if the circle we're deleting is a connection
            if let Ok(black_hole) = black_hole_query.get(id) {
                commands.entity(black_hole.wh).despawn_recursive();
            } else if let Ok(white_hole) = white_hole_query.get(id) {
                commands.entity(white_hole.bh).despawn_recursive();
            } else {
                // not a connection, despawn the holes on the other side
                for child in children.iter() {
                    if let Ok(black_hole) = black_hole_query.get(*child) {
                        commands.entity(black_hole.wh).despawn_recursive();
                    }
                    if let Ok(white_hole) = white_hole_query.get(*child) {
                        commands.entity(white_hole.bh).despawn_recursive();
                    }
                }
            }
            commands.entity(id).despawn_recursive();
        }
    }
}

// ------------------- connections -------------------

// hole enum?
#[derive(Component)]
struct WhiteHole {
    id: Entity,
    parent: Entity,
    bh: Entity,
    link_type: i32,
    changed: bool,
}

#[derive(Component)]
struct BlackHole {
    id: Entity,
    parent: Entity,
    wh: Entity,
    link_type: i32,
}

fn connect(
    mouse_button_input: Res<Input<MouseButton>>,
    mut commands: Commands,
    query: Query<(Entity, &Radius, &Transform), (With<Visible>, With<Order>)>,
    cursor: Res<CursorInfo>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    rad_query: Query<&Radius>,
    trans_query: Query<&Transform>,
) {
    if mouse_button_input.just_released(MouseButton::Left) {
        let mut source_entity: Option<Entity> = None;
        let mut sink_entity: Option<Entity> = None;
        for (e, r, t) in query.iter() {
            if cursor.i.distance(t.translation.xy()) < r.0 {
                source_entity = Some(e);
                continue;
            }
            if cursor.f.distance(t.translation.xy()) < r.0 {
                sink_entity = Some(e);
                continue;
            }
            if source_entity.is_some() && sink_entity.is_some() { break; }
        }

        if let (Some(src), Some(snk)) = (source_entity, sink_entity) {
            let src_radius = rad_query.get(src).unwrap().0;
            let snk_radius = rad_query.get(snk).unwrap().0;
            let src_trans = trans_query.get(src).unwrap().translation;
            let snk_trans = trans_query.get(snk).unwrap().translation;

            // spawn circles
            let black_hole = commands.spawn(( ColorMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(src_radius * 0.1).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::BLACK)),
                    transform: Transform::from_translation((cursor.i - src_trans.xy()).extend(0.000001)),
                    ..default()
                },
                Visible,
                Radius(src_radius * 0.1),
            )).id();
            let white_hole = commands.spawn(( ColorMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(snk_radius * 0.1).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::WHITE)),
                    transform: Transform::from_translation((cursor.f - snk_trans.xy()).extend(0.000001)),
                    ..default()
                },
                Visible,
                Radius(snk_radius * 0.1),
            )).id();

            // insert connection info
            commands.entity(black_hole).insert(
                BlackHole {
                    id: black_hole,
                    parent: src,
                    wh: white_hole,
                    link_type: 0,
                });
            commands.entity(white_hole).insert(
                WhiteHole {
                    id: white_hole,
                    parent: snk,
                    bh: black_hole,
                    link_type: 0,
                    changed: false,
                });

            // add to parents
            commands.entity(src).add_child(black_hole);
            commands.entity(snk).add_child(white_hole);

            // add link type text
            let black_hole_text = commands.spawn(Text2dBundle {
                text: Text::from_section(
                    0.to_string(),
                    TextStyle { color: Color::WHITE, ..default() },
                    ),
                transform: Transform::from_translation(Vec3{z:0.000001, ..default()}),
                ..default()
                }).id();
            commands.entity(black_hole).add_child(black_hole_text);

            let white_hole_text = commands.spawn(Text2dBundle {
                text: Text::from_section(
                    0.to_string(),
                    TextStyle { color: Color::BLACK, ..default() },
                    ),
                transform: Transform::from_translation(Vec3{z:0.000001, ..default()}),
                ..default()
                }).id();
            commands.entity(white_hole).add_child(white_hole_text);
        }
    }
}

fn draw_connections(
    mut gizmos: Gizmos,
    black_hole_query: Query<&BlackHole>,
    time: Res<Time>,
    trans_query: Query<&GlobalTransform>,
) {
    for black_hole in black_hole_query.iter() {
        let src_pos = trans_query.get(black_hole.id).unwrap().translation().xy();
        let snk_pos = trans_query.get(black_hole.wh).unwrap().translation().xy();
        let color = Color::hsl((time.elapsed_seconds() * 100.) % 360., 1.0, 0.5);
        gizmos.line_2d(src_pos, snk_pos, color);
    }
}

fn draw_connecting_line(
    mut gizmos: Gizmos,
    time: Res<Time>,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor: Res<CursorInfo>,
) {
    if mouse_button_input.pressed(MouseButton::Left) {
        let color = Color::hsl((time.elapsed_seconds() * 100.) % 360., 1.0, 0.5);
        gizmos.line_2d(cursor.i, cursor.f, color);
    }
}

fn update_link_type (
    keyboard_input: Res<Input<KeyCode>>,
    mut black_hole_query: Query<&mut BlackHole, With<Selected>>,
    mut white_hole_query: Query<&mut WhiteHole, With<Selected>>,
) {
    let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if shift && keyboard_input.just_pressed(KeyCode::Up) {
        for mut hole in black_hole_query.iter_mut() { hole.link_type += 1; }
        for mut hole in white_hole_query.iter_mut() { hole.link_type += 1; }
    }
    if shift && keyboard_input.just_pressed(KeyCode::Down) {
        for mut hole in black_hole_query.iter_mut() { hole.link_type -= 1; }
        for mut hole in white_hole_query.iter_mut() { hole.link_type -= 1; }
    }
}

fn update_link_type_text(
    mut query: Query<(&mut Text, &Parent), With<Visible>>,
    black_hole_query: Query<&BlackHole>,
    white_hole_query: Query<&WhiteHole>,
) {
    for (mut text, parent) in query.iter_mut() {
        if let Ok(hole) = black_hole_query.get(**parent) {
            text.sections[0].value = hole.link_type.to_string();
        }
        if let Ok(hole) = white_hole_query.get(**parent) {
            text.sections[0].value = hole.link_type.to_string();
        }
    }
}

// ------------------- process -----------------------

#[derive(Resource)]
struct BloomCircleId(Entity);

fn spawn_bloom_circle(
    mut commands: Commands,
    mut resource: ResMut<BloomCircleId>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let id = commands.spawn((
        ColorMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(100.).into()).into(),
            material: materials.add(ColorMaterial::from(Color::hsl(300., 1.0, 0.5))),
            transform: Transform::from_translation(Vec3{x:0., y:0., z:-11.}),
            ..default()
        },
        Radius(100.),
        Visible,
        Order(0),
        Num(0.),
        Arr(Vec::new()),
        Offset {trans:Vec3::ZERO, color:Color::BLACK, radius:0.},
    )).id();
    let text = commands.spawn(Text2dBundle {
        text: Text::from_sections([
            TextSection::new(
                "order: 0\n",
                TextStyle::default()
            ),
            TextSection::new(
                "0\n",
                TextStyle::default()
            ),
        ]),
        transform: Transform::from_translation(Vec3{z:0.000001, ..default()}),
        ..default()
    }).id();
    commands.entity(id).add_child(text);
    resource.0 = id;
}

fn update_bloom_settings(
    children_query: Query<&Children>,
    mut bloom: Query<&mut BloomSettings, With<Camera>>,
    black_hole_query: Query<&BlackHole>,
    mut white_hole_query: Query<&mut WhiteHole>,
    id: Res<BloomCircleId>,
    num_query: Query<&Num>,
) {
    let mut bloom_settings = bloom.single_mut();
    // why doesn't iter_descendants need error checking?
    for child in children_query.iter_descendants(id.0) {
        if let Ok(mut white_hole) = white_hole_query.get_mut(child) {
            if !white_hole.changed { continue; }
            white_hole.changed = false;
            let black_hole = black_hole_query.get(white_hole.bh).unwrap();
            let input = num_query.get(black_hole.parent).unwrap().0 / 100.;
            match (black_hole.link_type, white_hole.link_type) {
                (-4, 1) => bloom_settings.intensity = input,
                (-4, 2) => bloom_settings.low_frequency_boost = input,
                (-4, 3) => bloom_settings.low_frequency_boost_curvature = input,
                (-4, 4) => bloom_settings.high_pass_frequency = input,
                (-4, 5) => bloom_settings.composite_mode = if input > 0.5 {
                BloomCompositeMode::Additive } else { BloomCompositeMode::EnergyConserving },
                (-4, 6) => bloom_settings.prefilter_settings.threshold = input,
                (-4, 7) => bloom_settings.prefilter_settings.threshold_softness = input,
                _ => {},
            }
        }
    }
}

fn update_num_from_input(
    query: Query<(Entity, &Children)>,
    black_hole_query: Query<&BlackHole>,
    mut white_hole_query: Query<&mut WhiteHole>,
    mut num_query: Query<&mut Num>,
) {
    for (e, children) in query.iter() {
        for child in children.iter() {
            if let Ok(mut white_hole) = white_hole_query.get_mut(*child) {
                if !white_hole.changed { continue; }
                white_hole.changed = false;
                let black_hole = black_hole_query.get(white_hole.bh).unwrap();
                let input = num_query.get(black_hole.parent).unwrap().0;
                if black_hole.link_type == -4 && white_hole.link_type == -4 {
                    num_query.get_mut(e).unwrap().0 = input;
                    // now we have to let anything connected to this circle know about this change
                    for child in children.iter() {
                        if let Ok(black_hole) = black_hole_query.get(*child) {
                            if black_hole.link_type == -4 {
                                white_hole_query.get_mut(black_hole.wh).unwrap().changed = true;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn update_color_from_input(
    query: Query<(Entity, &Children)>,
    black_hole_query: Query<&BlackHole>,
    mut white_hole_query: Query<&mut WhiteHole>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    material_ids: Query<&Handle<ColorMaterial>>,
) {
    for (e, children) in query.iter() {
        for child in children.iter() {
            if let Ok(mut white_hole) = white_hole_query.get_mut(*child) {
                //if !white_hole.changed { continue; }
                //white_hole.changed = false;
                let black_hole = black_hole_query.get(white_hole.bh).unwrap();
                if black_hole.link_type == -2 && white_hole.link_type == -2 {
                    let id = material_ids.get(black_hole.parent).unwrap();
                    let mat = mats.get(id).unwrap();
                    let input = mat.color;
                    mats.get_mut(material_ids.get(e).unwrap()).unwrap().color = input;
                    for child in children.iter() {
                        if let Ok(black_hole) = black_hole_query.get(*child) {
                            if black_hole.link_type == -2 {
                                white_hole_query.get_mut(black_hole.wh).unwrap().changed = true;
                            }
                        }
                    }
                }
            }
        }
    }
}

