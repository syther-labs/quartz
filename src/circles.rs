use bevy::{
    prelude::*,
    render::view::{RenderLayers, VisibleEntities},
    sprite::{Mesh2dHandle, WithMesh2d},
    text::Text2dBounds,
};

use fundsp::net::Net;

use crate::{components::*, functions::*};

pub fn spawn_circles(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut depth: Local<f32>,
    cursor: Res<CursorInfo>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    default_color: Res<DefaultDrawColor>,
    default_verts: Res<DefaultDrawVerts>,
    mut polygon_handles: ResMut<PolygonHandles>,
) {
    if mouse_button_input.just_released(MouseButton::Left)
        && !keyboard_input.pressed(KeyCode::Space)
    {
        let r = cursor.f.distance(cursor.i);
        let v = default_verts.0;
        let color = default_color.0;
        if polygon_handles.0.len() <= v {
            polygon_handles.0.resize(v + 1, None);
        }
        if polygon_handles.0[v].is_none() {
            let handle = meshes.add(RegularPolygon::new(1., v)).into();
            polygon_handles.0[v] = Some(handle);
        }
        let (sndr, rcvr) = crossbeam_channel::bounded(1);
        commands.spawn((
            ColorMesh2dBundle {
                mesh: polygon_handles.0[v].clone().unwrap(),
                material: materials.add(ColorMaterial::from_color(color)),
                transform: Transform {
                    translation: cursor.i.extend(*depth),
                    scale: Vec3::new(r, r, 1.),
                    ..default()
                },
                ..default()
            },
            Vertices(v),
            Col(color),
            Number(0.),
            Arr(Vec::new()),
            Op("empty".to_string()),
            Targets(Vec::new()),
            Holes(Vec::new()),
            Order(0),
            (
                OpNum(0),
                Network(Net::new(0, 0)),
                NetIns(Vec::new()),
                OpChanged(false),
                LostWH(false),
                NetChannel(sndr, rcvr),
            ),
            RenderLayers::layer(1),
        ));
        *depth += 0.01;
    }
}

pub fn highlight_selected(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    selected: Query<(Entity, &Vertices, &Transform), (With<Selected>, Without<Highlight>)>,
    deselected: Query<Entity, (With<Highlight>, Without<Selected>)>,
    highlight_query: Query<&Highlight>,
    highlight_color: Res<HighlightColor>,
    polygon_handles: Res<PolygonHandles>,
) {
    for (e, v, t) in selected.iter() {
        let trans = t.translation.xy().extend(t.translation.z - 0.00001);
        let highlight = commands
            .spawn(ColorMesh2dBundle {
                mesh: polygon_handles.0[v.0].clone().unwrap(),
                material: materials.add(ColorMaterial::from_color(highlight_color.0)),
                transform: Transform {
                    translation: trans,
                    scale: Vec3::new(t.scale.x + 5., t.scale.y + 5., 1.),
                    rotation: t.rotation,
                },
                ..default()
            })
            .id();
        commands.entity(e).insert(Highlight(highlight));
    }
    for e in deselected.iter() {
        let highlight = highlight_query.get(e).unwrap();
        commands.entity(highlight.0).despawn();
        commands.entity(e).remove::<Highlight>();
    }
}

pub fn transform_highlights(
    moved: Query<(&Transform, &Highlight), Changed<Transform>>,
    changed_verts: Query<(&Vertices, &Highlight), Changed<Vertices>>,
    mut trans_query: Query<&mut Transform, Without<Highlight>>,
    mut handle_query: Query<&mut Mesh2dHandle>,
    polygon_handles: Res<PolygonHandles>,
) {
    for (t, h) in moved.iter() {
        // FIXME(amy): next_up/down would make offsets like this accurate
        // avoiding the funky behavior with bigger z values here
        let trans = t.translation.xy().extend(t.translation.z - 0.00001);
        trans_query.get_mut(h.0).unwrap().translation = trans;
        trans_query.get_mut(h.0).unwrap().rotation = t.rotation;
        trans_query.get_mut(h.0).unwrap().scale.x = t.scale.x + 5.;
        trans_query.get_mut(h.0).unwrap().scale.y = t.scale.y + 5.;
    }
    for (v, h) in changed_verts.iter() {
        if let Ok(mut handle) = handle_query.get_mut(h.0) {
            *handle = polygon_handles.0[v.0].clone().unwrap();
        }
    }
}

pub fn update_selection(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    circle_trans_query: Query<&Transform, With<Vertices>>,
    visible: Query<&VisibleEntities>,
    selected: Query<Entity, With<Selected>>,
    cursor: Res<CursorInfo>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut top_clicked_circle: Local<Option<(Entity, f32)>>,
    order_query: Query<(), With<Order>>, // non-hole circle
    mut clicked_on_space: ResMut<ClickedOnSpace>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    let shift = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let alt = keyboard_input.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if mouse_button_input.just_pressed(MouseButton::Left) {
        // NOTE apparently this can be missed in just_released?
        // so an invalid id when deleted. very rare tho (only 2 panics ever)
        // we only use the id here, so it's better updated here
        *top_clicked_circle = None;
        for e in visible.single().get::<WithMesh2d>() {
            if let Ok(t) = circle_trans_query.get(*e) {
                if top_clicked_circle.is_some() {
                    if t.translation.z > top_clicked_circle.unwrap().1
                        && cursor.i.distance_squared(t.translation.xy()) < t.scale.x * t.scale.x
                    {
                        *top_clicked_circle = Some((*e, t.translation.z));
                    }
                } else if cursor.i.distance_squared(t.translation.xy()) < t.scale.x * t.scale.x {
                    *top_clicked_circle = Some((*e, t.translation.z));
                }
            }
        }
        if let Some(top) = *top_clicked_circle {
            clicked_on_space.0 = false;
            if !selected.contains(top.0) {
                if shift {
                    commands.entity(top.0).insert(Selected);
                } else {
                    for entity in selected.iter() {
                        commands.entity(entity).remove::<Selected>();
                    }
                    commands.entity(top.0).insert(Selected);
                }
            } else if ctrl {
                commands.entity(top.0).remove::<Selected>();
            }
        } else {
            clicked_on_space.0 = true;
        }
    } else if mouse_button_input.just_released(MouseButton::Left) && top_clicked_circle.is_none() {
        if !shift {
            for entity in selected.iter() {
                commands.entity(entity).remove::<Selected>();
            }
        }
        // select those in the dragged area
        let (min_x, max_x) = if cursor.i.x < cursor.f.x {
            (cursor.i.x, cursor.f.x)
        } else {
            (cursor.f.x, cursor.i.x)
        };
        let (min_y, max_y) = if cursor.i.y < cursor.f.y {
            (cursor.i.y, cursor.f.y)
        } else {
            (cursor.f.y, cursor.i.y)
        };
        for e in visible.single().get::<WithMesh2d>() {
            if let Ok(t) = circle_trans_query.get(*e) {
                if (min_x < t.translation.x && t.translation.x < max_x)
                    && (min_y < t.translation.y && t.translation.y < max_y)
                {
                    // only select holes if ctrl is held
                    if (ctrl && order_query.contains(*e))
                    // only select non-holes if alt is held
                    || (alt && !order_query.contains(*e))
                    {
                        continue;
                    }
                    commands.entity(*e).insert(Selected);
                }
            }
        }
    }
}

pub fn move_selected(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorInfo>,
    mut circle_query: Query<&mut Transform, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    drag_modes: Res<DragModes>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if drag_modes.t {
        if mouse_button_input.pressed(MouseButton::Left)
            && !mouse_button_input.just_pressed(MouseButton::Left)
        {
            for mut t in circle_query.iter_mut() {
                t.translation.x += cursor.d.x;
                t.translation.y += cursor.d.y;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) {
            for mut t in circle_query.iter_mut() {
                t.translation.y += 1.;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) {
            for mut t in circle_query.iter_mut() {
                t.translation.y -= 1.;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowRight) {
            for mut t in circle_query.iter_mut() {
                t.translation.x += 1.;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            for mut t in circle_query.iter_mut() {
                t.translation.x -= 1.;
            }
        }
    }
}

pub fn rotate_selected(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorInfo>,
    mut query: Query<&mut Transform, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    drag_modes: Res<DragModes>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if drag_modes.o {
        if mouse_button_input.pressed(MouseButton::Left)
            && !mouse_button_input.just_pressed(MouseButton::Left)
        {
            for mut t in query.iter_mut() {
                t.rotate_z(cursor.d.y / 100.);
            }
        }
        if keyboard_input.any_pressed([KeyCode::ArrowUp, KeyCode::ArrowRight]) {
            for mut t in query.iter_mut() {
                t.rotate_z(0.01);
            }
        }
        if keyboard_input.any_pressed([KeyCode::ArrowDown, KeyCode::ArrowLeft]) {
            for mut t in query.iter_mut() {
                t.rotate_z(-0.01);
            }
        }
    }
}

pub fn update_color(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorInfo>,
    mut query: Query<&mut Col, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    drag_modes: Res<DragModes>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if mouse_button_input.pressed(MouseButton::Left)
        && !mouse_button_input.just_pressed(MouseButton::Left)
    {
        if drag_modes.h {
            for mut c in query.iter_mut() {
                let h = (c.0.hue + cursor.d.x).clamp(0., 360.);
                c.0.hue = h;
            }
        }
        if drag_modes.s {
            for mut c in query.iter_mut() {
                let s = (c.0.saturation + cursor.d.x / 100.).clamp(0., 1.);
                c.0.saturation = s;
            }
        }
        if drag_modes.l {
            for mut c in query.iter_mut() {
                let l = (c.0.lightness + cursor.d.x / 100.).clamp(0., 1.);
                c.0.lightness = l;
            }
        }
        if drag_modes.a {
            for mut c in query.iter_mut() {
                let a = (c.0.alpha + cursor.d.x / 100.).clamp(0., 1.);
                c.0.alpha = a;
            }
        }
    }
    if keyboard_input.any_pressed([KeyCode::ArrowLeft, KeyCode::ArrowDown]) {
        for mut c in query.iter_mut() {
            if drag_modes.h {
                let h = (c.0.hue - 1.).clamp(0., 360.);
                c.0.hue = h;
            }
            if drag_modes.s {
                let s = (c.0.saturation - 0.01).clamp(0., 1.);
                c.0.saturation = s;
            }
            if drag_modes.l {
                let l = (c.0.lightness - 0.01).clamp(0., 1.);
                c.0.lightness = l;
            }
            if drag_modes.a {
                let a = (c.0.alpha - 0.01).clamp(0., 1.);
                c.0.alpha = a;
            }
        }
    }
    if keyboard_input.any_pressed([KeyCode::ArrowRight, KeyCode::ArrowUp]) {
        for mut c in query.iter_mut() {
            if drag_modes.h {
                let h = (c.0.hue + 1.).clamp(0., 360.);
                c.0.hue = h;
            }
            if drag_modes.s {
                let s = (c.0.saturation + 0.01).clamp(0., 1.);
                c.0.saturation = s;
            }
            if drag_modes.l {
                let l = (c.0.lightness + 0.01).clamp(0., 1.);
                c.0.lightness = l;
            }
            if drag_modes.a {
                let a = (c.0.alpha + 0.01).clamp(0., 1.);
                c.0.alpha = a;
            }
        }
    }
}

pub fn update_mat(
    mut mats: ResMut<Assets<ColorMaterial>>,
    material_ids: Query<&Handle<ColorMaterial>>,
    color_query: Query<(Entity, &Col), Changed<Col>>,
) {
    for (id, c) in color_query.iter() {
        if let Ok(mat_id) = material_ids.get(id) {
            let mat = mats.get_mut(mat_id).unwrap();
            mat.color = Color::Hsla(c.0);
        }
    }
}

pub fn update_radius(
    mut query: Query<&mut Transform, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    cursor: Res<CursorInfo>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    drag_modes: Res<DragModes>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if drag_modes.r {
        if mouse_button_input.pressed(MouseButton::Left)
            && !mouse_button_input.just_pressed(MouseButton::Left)
        {
            for mut t in query.iter_mut() {
                t.scale.x = (t.scale.x + cursor.d.y).max(0.);
                t.scale.y = (t.scale.y + cursor.d.y).max(0.);
            }
        }
        if keyboard_input.any_pressed([KeyCode::ArrowUp, KeyCode::ArrowRight]) {
            for mut t in query.iter_mut() {
                t.scale.x = (t.scale.x + 1.).max(0.);
                t.scale.y = (t.scale.y + 1.).max(0.);
            }
        }
        if keyboard_input.any_pressed([KeyCode::ArrowDown, KeyCode::ArrowLeft]) {
            for mut t in query.iter_mut() {
                t.scale.x = (t.scale.x - 1.).max(0.);
                t.scale.y = (t.scale.y - 1.).max(0.);
            }
        }
    }
}

pub fn update_vertices(
    mut query: Query<&mut Vertices, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    drag_modes: Res<DragModes>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorInfo>,
    mut delta: Local<f32>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if drag_modes.v {
        if mouse_button_input.pressed(MouseButton::Left)
            && !mouse_button_input.just_pressed(MouseButton::Left)
        {
            *delta += cursor.d.y / 10.;
            let d = *delta as i32;
            if d >= 1 {
                for mut v in query.iter_mut() {
                    v.0 = (v.0 as i32 + d).min(64) as usize;
                }
                *delta = 0.;
            } else if d <= -1 {
                for mut v in query.iter_mut() {
                    v.0 = (v.0 as i32 + d).max(3) as usize;
                }
                *delta = 0.;
            }
        }
        if keyboard_input.any_just_pressed([KeyCode::ArrowUp, KeyCode::ArrowRight]) {
            for mut v in query.iter_mut() {
                v.0 = (v.0 + 1).min(64);
            }
        }
        if keyboard_input.any_just_pressed([KeyCode::ArrowDown, KeyCode::ArrowLeft]) {
            for mut v in query.iter_mut() {
                v.0 = (v.0 - 1).max(3);
            }
        }
    }
}

pub fn update_mesh(
    mut meshes: ResMut<Assets<Mesh>>,
    mut handle_query: Query<&mut Mesh2dHandle>,
    query: Query<(Entity, &Vertices), Changed<Vertices>>,
    mut polygon_handles: ResMut<PolygonHandles>,
) {
    for (id, v) in query.iter() {
        if polygon_handles.0.len() <= v.0 {
            polygon_handles.0.resize(v.0 + 1, None);
        }
        if polygon_handles.0[v.0].is_none() {
            let handle = meshes.add(RegularPolygon::new(1., v.0)).into();
            polygon_handles.0[v.0] = Some(handle);
        }
        if let Ok(mut handle) = handle_query.get_mut(id) {
            *handle = polygon_handles.0[v.0].clone().unwrap();
        }
    }
}

pub fn update_num(
    mut query: Query<&mut Number, With<Selected>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    cursor: Res<CursorInfo>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    drag_modes: Res<DragModes>,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        return;
    }
    if drag_modes.n {
        if mouse_button_input.pressed(MouseButton::Left)
            && !mouse_button_input.just_pressed(MouseButton::Left)
        {
            for mut n in query.iter_mut() {
                n.0 += cursor.d.y / 10.;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) {
            for mut n in query.iter_mut() {
                n.0 += 0.01;
            }
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) {
            for mut n in query.iter_mut() {
                n.0 -= 0.01;
            }
        }
    }
}

pub fn spawn_info_text(
    show_info_text: Res<ShowInfoText>,
    // fully loaded (not an orphaned hole) that's why With<RenderLayers>
    query: Query<Entity, (With<Vertices>, Without<InfoText>, With<RenderLayers>)>,
    text_size: Res<TextSize>,
    mut commands: Commands,
) {
    if show_info_text.0 {
        for e in query.iter() {
            let id_text = if show_info_text.1 { format!("{}\n", e) } else { String::new() };
            let info_text = commands
                .spawn(Text2dBundle {
                    text: Text::from_sections([
                        TextSection::new(
                            id_text,
                            TextStyle { color: Color::BLACK, font_size: 120., ..default() },
                        ),
                        TextSection::new(
                            "",
                            TextStyle { color: Color::BLACK, font_size: 120., ..default() },
                        ),
                        TextSection::new(
                            "",
                            TextStyle { color: Color::BLACK, font_size: 120., ..default() },
                        ),
                        TextSection::new(
                            "",
                            TextStyle { color: Color::BLACK, font_size: 120., ..default() },
                        ),
                    ])
                    .with_justify(JustifyText::Left),
                    transform: Transform::from_scale(Vec3::new(text_size.0, text_size.0, 1.)),
                    ..default()
                })
                .id();
            commands.entity(e).insert(InfoText(info_text));
        }
    }
}

pub fn update_info_text(
    mut query: Query<(Entity, &mut InfoText)>,
    mut text_query: Query<&mut Text>,
    mut text_bounds: Query<&mut Text2dBounds>,
    mut trans_query: Query<&mut Transform>,
    mut order_query: Query<&mut Order>,
    mut num_query: Query<&mut Number>,
    mut op_query: Query<&mut Op>,
    mut white_hole_query: Query<&mut WhiteHole>,
    black_hole_query: Query<&BlackHole>,
    mut color_query: Query<&mut Col>,
    text_size: Res<TextSize>,
) {
    for (id, info) in query.iter_mut() {
        let t = trans_query.get_mut(id).unwrap();
        if t.is_changed() || info.is_added() || text_size.is_changed() {
            text_bounds.get_mut(info.0).unwrap().size.x = t.scale.x * text_size.0.recip();
            let t = t.translation;
            trans_query.get_mut(info.0).unwrap().translation = t.xy().extend(t.z + 0.00001);
        }
        if let Ok(ord) = order_query.get_mut(id) {
            if ord.is_changed() || info.is_added() {
                text_query.get_mut(info.0).unwrap().sections[1].value = format!("{}\n", ord.0);
            }
        }
        if let Ok(n) = num_query.get_mut(id) {
            if n.is_changed() || info.is_added() {
                text_query.get_mut(info.0).unwrap().sections[3].value = n.0.to_string();
            }
        }
        if let Ok(op) = op_query.get_mut(id) {
            if op.is_changed() || info.is_added() {
                text_query.get_mut(info.0).unwrap().sections[2].value = format!("{}\n", op.0);
            }
        }
        if let Ok(wh) = white_hole_query.get_mut(id) {
            if wh.is_changed() || info.is_added() {
                text_query.get_mut(info.0).unwrap().sections[1].value =
                    lt_to_string(wh.link_types.1);
            }
        }
        if let Ok(bh) = black_hole_query.get(id) {
            if let Ok(wh) = white_hole_query.get_mut(bh.wh) {
                if wh.is_changed() || info.is_added() {
                    text_query.get_mut(info.0).unwrap().sections[1].value =
                        lt_to_string(wh.link_types.0);
                }
            }
        }
        let c = color_query.get_mut(id).unwrap();
        if c.is_changed() || info.is_added() {
            let l = if c.0.lightness < 0.3 { 1. } else { 0. };
            let opposite_color = Color::hsl(0., 1.0, l);
            let t = &mut text_query.get_mut(info.0).unwrap();
            for section in &mut t.sections {
                section.style.color = opposite_color;
            }
        }
    }
}

pub fn delete_selected(
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
    mut holes_query: Query<&mut Holes>,
    bh_query: Query<&BlackHole>,
    wh_query: Query<&WhiteHole>,
    arrow_query: Query<&ConnectionArrow>,
    info_text_query: Query<&InfoText>,
    highlight_query: Query<&Highlight>,
    mut order_change: EventWriter<OrderChange>,
    mut lost_wh_query: Query<&mut LostWH>,
) {
    let mut order = false;
    for e in selected_query.iter() {
        if let Ok(holes) = holes_query.get(e) {
            // it's a circle
            for hole in &holes.0.clone() {
                if let Ok(bh) = bh_query.get(*hole) {
                    let arrow = arrow_query.get(bh.wh).unwrap().0;
                    commands.entity(arrow).despawn();
                    commands.entity(*hole).despawn();
                    commands.entity(bh.wh).despawn();
                    if let Ok(wh_text) = info_text_query.get(bh.wh) {
                        commands.entity(wh_text.0).despawn();
                    }
                    if let Ok(bh_text) = info_text_query.get(*hole) {
                        commands.entity(bh_text.0).despawn();
                    }
                    if let Ok(highlight) = highlight_query.get(bh.wh) {
                        commands.entity(highlight.0).despawn();
                    }
                    if let Ok(highlight) = highlight_query.get(*hole) {
                        commands.entity(highlight.0).despawn();
                    }
                    lost_wh_query.get_mut(bh.wh_parent).unwrap().0 = true;
                    holes_query.get_mut(bh.wh_parent).unwrap().0.retain(|x| *x != bh.wh);
                } else if let Ok(wh) = wh_query.get(*hole) {
                    // don't remove things that will get removed later
                    if selected_query.contains(wh.bh_parent) {
                        continue;
                    }
                    let arrow = arrow_query.get(*hole).unwrap().0;
                    commands.entity(arrow).despawn();
                    commands.entity(wh.bh).despawn();
                    commands.entity(*hole).despawn();
                    if let Ok(wh_text) = info_text_query.get(*hole) {
                        commands.entity(wh_text.0).despawn();
                    }
                    if let Ok(bh_text) = info_text_query.get(wh.bh) {
                        commands.entity(bh_text.0).despawn();
                    }
                    if let Ok(highlight) = highlight_query.get(*hole) {
                        commands.entity(highlight.0).despawn();
                    }
                    if let Ok(highlight) = highlight_query.get(wh.bh) {
                        commands.entity(highlight.0).despawn();
                    }
                    holes_query.get_mut(wh.bh_parent).unwrap().0.retain(|x| *x != wh.bh);
                }
            }
            order = true;
            if let Ok(text) = info_text_query.get(e) {
                commands.entity(text.0).despawn();
            }
            if let Ok(highlight) = highlight_query.get(e) {
                commands.entity(highlight.0).despawn();
            }
            commands.entity(e).despawn();
        } else {
            // it's a hole
            if let Ok(wh) = wh_query.get(e) {
                // get parent
                let parent = bh_query.get(wh.bh).unwrap().wh_parent;
                if selected_query.contains(parent) {
                    continue;
                }
                if selected_query.contains(wh.bh_parent) {
                    continue;
                }
                // remove from parents' vecs
                holes_query.get_mut(parent).unwrap().0.retain(|x| *x != e);
                holes_query.get_mut(wh.bh_parent).unwrap().0.retain(|x| *x != wh.bh);
                // parent has lost a wh
                lost_wh_query.get_mut(parent).unwrap().0 = true;
                let arrow = arrow_query.get(e).unwrap().0;
                commands.entity(arrow).despawn();
                commands.entity(e).despawn();
                commands.entity(wh.bh).despawn();
                // info texts and highlights
                if let Ok(wh_text) = info_text_query.get(e) {
                    commands.entity(wh_text.0).despawn();
                }
                if let Ok(bh_text) = info_text_query.get(wh.bh) {
                    commands.entity(bh_text.0).despawn();
                }
                if let Ok(highlight) = highlight_query.get(e) {
                    commands.entity(highlight.0).despawn();
                }
                if let Ok(highlight) = highlight_query.get(wh.bh) {
                    commands.entity(highlight.0).despawn();
                }
            } else if let Ok(bh) = bh_query.get(e) {
                let parent = wh_query.get(bh.wh).unwrap().bh_parent;
                if selected_query.contains(parent) {
                    continue;
                }
                if selected_query.contains(bh.wh_parent) {
                    continue;
                }
                if selected_query.contains(bh.wh) {
                    continue;
                }
                holes_query.get_mut(parent).unwrap().0.retain(|x| *x != e);
                holes_query.get_mut(bh.wh_parent).unwrap().0.retain(|x| *x != bh.wh);
                lost_wh_query.get_mut(bh.wh_parent).unwrap().0 = true;
                let arrow = arrow_query.get(bh.wh).unwrap().0;
                commands.entity(arrow).despawn();
                commands.entity(e).despawn();
                commands.entity(bh.wh).despawn();
                if let Ok(wh_text) = info_text_query.get(e) {
                    commands.entity(wh_text.0).despawn();
                }
                if let Ok(bh_text) = info_text_query.get(bh.wh) {
                    commands.entity(bh_text.0).despawn();
                }
                if let Ok(highlight) = highlight_query.get(e) {
                    commands.entity(highlight.0).despawn();
                }
                if let Ok(highlight) = highlight_query.get(bh.wh) {
                    commands.entity(highlight.0).despawn();
                }
            }
        }
    }
    if order {
        order_change.send_default();
    }
}

pub fn open_after_drag(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    drag_modes: Res<DragModes>,
    query: Query<&Holes, With<Selected>>,
    mut white_hole_query: Query<&mut WhiteHole>,
    black_hole_query: Query<&BlackHole>,
) {
    let arrows = [KeyCode::ArrowDown, KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::ArrowRight];
    if keyboard_input.any_pressed(arrows) || mouse_button_input.pressed(MouseButton::Left) {
        let mut lts_to_open = Vec::new();
        if drag_modes.t {
            lts_to_open.push(-3);
            lts_to_open.push(-4);
        }
        if drag_modes.r {
            lts_to_open.push(-2);
        }
        if drag_modes.n {
            lts_to_open.push(-1);
        }
        if drag_modes.h {
            lts_to_open.push(-6);
        }
        if drag_modes.s {
            lts_to_open.push(-7);
        }
        if drag_modes.l {
            lts_to_open.push(-8);
        }
        if drag_modes.a {
            lts_to_open.push(-9);
        }
        if drag_modes.o {
            lts_to_open.push(-12);
        }
        if drag_modes.v {
            lts_to_open.push(-11);
        }
        for holes in query.iter() {
            for hole in &holes.0 {
                if let Ok(bh) = black_hole_query.get(*hole) {
                    if let Ok(wh) = white_hole_query.get(bh.wh) {
                        if lts_to_open.contains(&wh.link_types.0) {
                            white_hole_query.get_mut(bh.wh).unwrap().open = true;
                        }
                    }
                }
            }
        }
    }
}
