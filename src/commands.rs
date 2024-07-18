use bevy::{
    app::AppExit,
    core_pipeline::bloom::{BloomCompositeMode, BloomSettings},
    ecs::system::SystemParam,
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    render::view::{RenderLayers, VisibleEntities},
    sprite::WithMesh2d,
};

use crate::{components::*, functions::*};

use fundsp::audiounit::AudioUnit;

use copypasta::ClipboardProvider;

use cpal::traits::{DeviceTrait, HostTrait};

#[derive(SystemParam)]
pub struct Access<'w, 's> {
    op_query: Query<'w, 's, &'static mut Op>,
    num_query: Query<'w, 's, &'static mut Number>,
    col_query: Query<'w, 's, &'static mut Col>,
    trans_query: Query<'w, 's, &'static mut Transform>,
    arr_query: Query<'w, 's, &'static mut Arr>,
    order_query: Query<'w, 's, &'static mut Order>,
    selected_query: Query<'w, 's, Entity, With<Selected>>,
    white_hole_query: Query<'w, 's, &'static mut WhiteHole>,
    black_hole_query: Query<'w, 's, &'static mut BlackHole>,
    order_change: EventWriter<'w, OrderChange>,
    vertices_query: Query<'w, 's, &'static mut Vertices>,
    save_event: EventWriter<'w, SaveCommand>,
    copy_event: EventWriter<'w, CopyCommand>,
    delete_event: EventWriter<'w, DeleteCommand>,
    targets_query: Query<'w, 's, &'static mut Targets>,
    render_layers: Query<'w, 's, &'static mut RenderLayers, With<Camera>>,
    net_query: Query<'w, 's, &'static mut Network>,
    op_changed_query: Query<'w, 's, &'static mut OpChanged>,
    exit_event: EventWriter<'w, AppExit>,
    drag_modes: ResMut<'w, DragModes>,
    default_color: ResMut<'w, DefaultDrawColor>,
    default_verts: ResMut<'w, DefaultDrawVerts>,
    default_lt: ResMut<'w, DefaultLT>,
    version: Res<'w, Version>,
    visible: Query<'w, 's, &'static VisibleEntities>,
    out_device_event: EventWriter<'w, OutDeviceCommand>,
    in_device_event: EventWriter<'w, InDeviceCommand>,
    node_limit: ResMut<'w, NodeLimit>,
    op_num_query: Query<'w, 's, &'static mut OpNum>,
    clipboard: ResMut<'w, SystemClipboard>,
    paste_chan: Res<'w, PasteChannel>,
    bloom: Query<'w, 's, &'static mut BloomSettings, With<Camera>>,
    show_info_text: ResMut<'w, ShowInfoText>,
}

pub fn command_parser(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut key_event: EventReader<KeyboardInput>,
    mut command_line_text: Query<&mut Text, With<CommandText>>,
    circle_query: Query<Entity, With<Vertices>>,
    mut access: Access,
    mut next_mode: ResMut<NextState<Mode>>,
    mode: Res<State<Mode>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    info_text_query: Query<(Entity, &InfoText)>,
    holes_query: Query<&Holes>,
) {
    let clt = &mut command_line_text.single_mut();
    if key_event.is_empty() && !clt.is_changed() && !keyboard_input.just_released(KeyCode::KeyT) {
        return;
    }

    let text = &mut clt.sections[0].value;

    if *mode.get() == Mode::Draw {
        // exit to edit
        if keyboard_input.any_just_pressed([KeyCode::Escape, KeyCode::KeyE]) {
            text.clear();
            next_mode.set(Mode::Edit);
            key_event.clear(); // we have an 'e' that we don't want
        }
        // switch to connect mode
        if keyboard_input.just_pressed(KeyCode::KeyC) {
            *text = "-- CONNECT --".to_string();
            next_mode.set(Mode::Connect);
        }
    } else if *mode.get() == Mode::Connect {
        // edit the link type
        if text.starts_with("-- CONNECT") && keyboard_input.just_pressed(KeyCode::KeyC) {
            *text = "-- LT --> ".to_string();
            return;
        }
        if text.starts_with("-- LT --> ") {
            for key in key_event.read() {
                if key.state.is_pressed() {
                    if let Key::Character(c) = &key.logical_key {
                        if text.len() == 10 {
                            access.default_lt.0 .0 = str_to_lt(c);
                            text.push_str(c);
                        } else if text.len() == 11 {
                            access.default_lt.0 .1 = str_to_lt(c);
                            *text = "-- CONNECT --".to_string();
                        }
                    }
                }
            }
            return;
        }
        if !keyboard_input.pressed(KeyCode::KeyT) {
            *text = format!(
                "-- CONNECT -- ({} {})",
                lt_to_string(access.default_lt.0 .0),
                lt_to_string(access.default_lt.0 .1)
            );
        }
        // exit to edit
        if keyboard_input.any_just_pressed([KeyCode::Escape, KeyCode::KeyE]) {
            text.clear();
            next_mode.set(Mode::Edit);
            key_event.clear(); // consume the 'e' when exiting to edit
        }
        // target
        if keyboard_input.just_pressed(KeyCode::KeyT) {
            *text = "-- TARGET --".to_string();
        }
        // switch to draw mode
        if keyboard_input.just_pressed(KeyCode::KeyD) {
            *text = "-- DRAW --".to_string();
            next_mode.set(Mode::Draw);
        }
    } else if *mode.get() == Mode::Edit {
        if keyboard_input.just_pressed(KeyCode::Delete) {
            access.delete_event.send_default();
            return;
        }

        for key in key_event.read() {
            if key.state.is_pressed() {
                match &key.logical_key {
                    Key::Character(c) => {
                        if let Some(c) = c.chars().next() {
                            if text.starts_with('>') {
                                text.clear();
                            }
                            if !c.is_control() && *text != "F" {
                                text.push(c);
                            }
                        }
                    }
                    Key::Space => {
                        if !text.ends_with(' ') && !text.is_empty() && *text != "F" {
                            text.push(' ');
                        }
                    }
                    Key::Backspace => {
                        text.pop();
                    }
                    Key::Escape => {
                        text.clear();
                    }
                    Key::Enter => {
                        text.push('\t');
                    }
                    // tab completion when?
                    _ => {}
                }
            }
        }
        if text.ends_with('\t') {
            // commands starting with :
            let lines = text.as_str().split(';');
            for line in lines {
                // (entity, lt) if there's a given entity
                let mut lt_to_open = (None, None);
                let mut command = line.split_ascii_whitespace();
                let c0 = command.next();
                match c0 {
                    // open scene file
                    Some(":e") => {
                        if let Some(s) = command.next() {
                            commands.spawn(DynamicSceneBundle {
                                scene: asset_server.load(s.to_string()),
                                ..default()
                            });
                        }
                    }
                    // save scene file
                    Some(":w") => {
                        if let Some(s) = command.next() {
                            access.save_event.send(SaveCommand(s.to_string()));
                        }
                    }
                    Some(":q") => {
                        access.exit_event.send_default();
                    }
                    Some(":od") | Some(":id") => {
                        let h = command.next();
                        let d = command.next();
                        let mut sr = None;
                        let mut b = None;
                        if let (Some(h), Some(d)) = (h, d) {
                            let h = h.parse::<usize>();
                            let d = d.parse::<usize>();
                            if let (Ok(h), Ok(d)) = (h, d) {
                                let samplerate = command.next();
                                let block = command.next();
                                if let Some(samplerate) = samplerate {
                                    if let Ok(samplerate) = samplerate.parse::<u32>() {
                                        sr = Some(samplerate);
                                    }
                                }
                                if let Some(block) = block {
                                    if let Ok(block) = block.parse::<u32>() {
                                        b = Some(block);
                                    }
                                }
                                if c0 == Some(":od") {
                                    access.out_device_event.send(OutDeviceCommand(h, d, sr, b));
                                } else {
                                    access.in_device_event.send(InDeviceCommand(h, d, sr, b));
                                }
                            }
                        }
                    }
                    Some(":nl") => {
                        if let Some(s) = command.next() {
                            if let Ok(n) = s.parse::<usize>() {
                                access.node_limit.0 = n;
                            }
                        }
                    }
                    // white hole / black hole link type
                    Some(":lt") | Some("lt") => {
                        if let Some(s) = command.next() {
                            if let Some(e) = str_to_id(s) {
                                if let Ok(mut wh) = access.white_hole_query.get_mut(e) {
                                    if let Some(s) = command.next() {
                                        wh.link_types.1 = str_to_lt(s);
                                        wh.open = true;
                                    }
                                } else if let Ok(bh) = access.black_hole_query.get(e) {
                                    let wh = &mut access.white_hole_query.get_mut(bh.wh).unwrap();
                                    if let Some(s) = command.next() {
                                        wh.link_types.0 = str_to_lt(s);
                                        wh.open = true;
                                    }
                                }
                            } else {
                                for id in access.selected_query.iter() {
                                    if let Ok(mut wh) = access.white_hole_query.get_mut(id) {
                                        wh.link_types.1 = str_to_lt(s);
                                        wh.open = true;
                                    } else if let Ok(bh) = access.black_hole_query.get(id) {
                                        let wh =
                                            &mut access.white_hole_query.get_mut(bh.wh).unwrap();
                                        wh.link_types.0 = str_to_lt(s);
                                        wh.open = true;
                                    }
                                }
                            }
                        }
                    }
                    Some(":dv") | Some("dv") => {
                        if let Some(s) = command.next() {
                            if let Ok(n) = s.parse::<usize>() {
                                access.default_verts.0 = n.clamp(3, 64);
                            }
                        }
                    }
                    Some(":dc") | Some("dc") => {
                        let mut h = 270.;
                        let mut s = 1.;
                        let mut l = 0.5;
                        let mut a = 1.;
                        if let Some(n) = command.next() {
                            if let Ok(n) = n.parse::<f32>() {
                                h = n;
                            }
                        }
                        if let Some(n) = command.next() {
                            if let Ok(n) = n.parse::<f32>() {
                                s = n;
                            }
                        }
                        if let Some(n) = command.next() {
                            if let Ok(n) = n.parse::<f32>() {
                                l = n;
                            }
                        }
                        if let Some(n) = command.next() {
                            if let Ok(n) = n.parse::<f32>() {
                                a = n;
                            }
                        }
                        access.default_color.0 = Hsla::new(h, s, l, a);
                    }
                    // toggle open a white hole (by id)
                    Some(":ht") | Some("ht") => {
                        if let Some(s) = command.next() {
                            if let Some(e) = str_to_id(s) {
                                if let Ok(mut wh) = access.white_hole_query.get_mut(e) {
                                    wh.open = !wh.open;
                                }
                            }
                        }
                    }
                    Some(":push") | Some("push") => {
                        if let Some(a1) = command.next() {
                            if let Some(a2) = command.next() {
                                if let Some(e) = str_to_id(a1) {
                                    if let Some(t) = str_to_id(a2) {
                                        if let Ok(mut targets) = access.targets_query.get_mut(e) {
                                            targets.0.push(t);
                                        }
                                    } else if let Ok(n) = parse_with_constants(a2) {
                                        if let Ok(mut arr) = access.arr_query.get_mut(e) {
                                            arr.0.push(n);
                                        }
                                    }
                                }
                            } else {
                                for id in access.selected_query.iter() {
                                    if let Some(t) = str_to_id(a1) {
                                        if let Ok(mut targets) = access.targets_query.get_mut(id) {
                                            targets.0.push(t);
                                        }
                                    } else if let Ok(n) = parse_with_constants(a1) {
                                        if let Ok(mut arr) = access.arr_query.get_mut(id) {
                                            arr.0.push(n);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(":set") | Some("set") | Some(":delta") | Some("delta") => {
                        let c1 = command.next();
                        match c1 {
                            Some("n") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut num) = access.num_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        num.0 = n;
                                                    } else {
                                                        num.0 += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-1));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut num) = access.num_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    num.0 = n;
                                                } else {
                                                    num.0 += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-1));
                                    }
                                }
                            }
                            Some("r") | Some("rx") | Some("ry") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut trans) = access.trans_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        if c1 == Some("r") {
                                                            trans.scale.x = n.max(0.);
                                                            trans.scale.y = n.max(0.);
                                                        } else if c1 == Some("rx") {
                                                            trans.scale.x = n.max(0.);
                                                        } else {
                                                            trans.scale.y = n.max(0.);
                                                        }
                                                    } else if c1 == Some("r") {
                                                        trans.scale.x = (trans.scale.x + n).max(0.);
                                                        trans.scale.y = (trans.scale.y + n).max(0.);
                                                    } else if c1 == Some("rx") {
                                                        trans.scale.x = (trans.scale.x + n).max(0.);
                                                    } else {
                                                        trans.scale.y = (trans.scale.y + n).max(0.);
                                                    }
                                                    lt_to_open = (Some(e), Some(-2));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut trans) = access.trans_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    if c1 == Some("r") {
                                                        trans.scale.x = n.max(0.);
                                                        trans.scale.y = n.max(0.);
                                                    } else if c1 == Some("rx") {
                                                        trans.scale.x = n.max(0.);
                                                    } else {
                                                        trans.scale.y = n.max(0.);
                                                    }
                                                } else if c1 == Some("r") {
                                                    trans.scale.x = (trans.scale.x + n).max(0.);
                                                    trans.scale.y = (trans.scale.y + n).max(0.);
                                                } else if c1 == Some("rx") {
                                                    trans.scale.x = (trans.scale.x + n).max(0.);
                                                } else {
                                                    trans.scale.y = (trans.scale.y + n).max(0.);
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-2));
                                    }
                                }
                            }
                            Some("x") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut t) = access.trans_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        t.translation.x = n;
                                                    } else {
                                                        t.translation.x += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-3));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut t) = access.trans_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    t.translation.x = n;
                                                } else {
                                                    t.translation.x += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-3));
                                    }
                                }
                            }
                            Some("y") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut t) = access.trans_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        t.translation.y = n;
                                                    } else {
                                                        t.translation.y += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-4));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut t) = access.trans_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    t.translation.y = n;
                                                } else {
                                                    t.translation.y += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-4));
                                    }
                                }
                            }
                            Some("z") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut t) = access.trans_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        t.translation.z = n;
                                                    } else {
                                                        t.translation.z += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-5));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut t) = access.trans_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    t.translation.z = n;
                                                } else {
                                                    t.translation.z += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-5));
                                    }
                                }
                            }
                            Some("h") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut color) = access.col_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        color.0.hue = n;
                                                    } else {
                                                        color.0.hue += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-6));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut color) = access.col_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    color.0.hue = n;
                                                } else {
                                                    color.0.hue += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-6));
                                    }
                                }
                            }
                            Some("s") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut color) = access.col_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        color.0.saturation = n;
                                                    } else {
                                                        color.0.saturation += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-7));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut color) = access.col_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    color.0.saturation = n;
                                                } else {
                                                    color.0.saturation += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-7));
                                    }
                                }
                            }
                            Some("l") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut color) = access.col_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        color.0.lightness = n;
                                                    } else {
                                                        color.0.lightness += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-8));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut color) = access.col_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    color.0.lightness = n;
                                                } else {
                                                    color.0.lightness += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-8));
                                    }
                                }
                            }
                            Some("a") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut color) = access.col_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        color.0.alpha = n;
                                                    } else {
                                                        color.0.alpha += n;
                                                    }
                                                    lt_to_open = (Some(e), Some(-9));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut color) = access.col_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    color.0.alpha = n;
                                                } else {
                                                    color.0.alpha += n;
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-9));
                                    }
                                }
                            }
                            Some("v") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut vertices) = access.vertices_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = n.parse::<usize>() {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        vertices.0 = n.max(3);
                                                    } else {
                                                        vertices.0 = (vertices.0 + n).max(3);
                                                    }
                                                    lt_to_open = (Some(e), Some(-11));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = s.parse::<usize>() {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut vertices) =
                                                access.vertices_query.get_mut(id)
                                            {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    vertices.0 = n.max(3);
                                                } else {
                                                    vertices.0 = (vertices.0 + n).max(3);
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-11));
                                    }
                                }
                            }
                            Some("o") | Some("rot") | Some("rotation") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut t) = access.trans_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = parse_with_constants(n) {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        t.rotation = Quat::from_rotation_z(n);
                                                    } else {
                                                        let rot =
                                                            t.rotation.to_euler(EulerRot::XYZ).2;
                                                        t.rotation = Quat::from_rotation_z(rot + n);
                                                    }
                                                    lt_to_open = (Some(e), Some(-12));
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = parse_with_constants(s) {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut t) = access.trans_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    t.rotation = Quat::from_rotation_z(n);
                                                } else {
                                                    let rot = t.rotation.to_euler(EulerRot::XYZ).2;
                                                    t.rotation = Quat::from_rotation_z(rot + n);
                                                }
                                            }
                                        }
                                        lt_to_open = (None, Some(-12));
                                    }
                                }
                            }
                            Some("op") => {
                                if let Some(s) = command.next() {
                                    let op_str = line.trim(); // we have a \t at the end
                                    let op_str = if op_str.starts_with(':') {
                                        op_str.trim_start_matches(":set op ")
                                    } else {
                                        op_str.trim_start_matches("set op ")
                                    };
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut op) = access.op_query.get_mut(e) {
                                            let op_str = op_str.trim_start_matches(s).trim_start();
                                            op.0 = op_str.into();
                                            access.op_changed_query.get_mut(e).unwrap().0 = true;
                                            access.net_query.get_mut(e).unwrap().0 =
                                                str_to_net(op_str);
                                            access.op_num_query.get_mut(e).unwrap().0 =
                                                str_to_op_num(op_str);
                                            lt_to_open = (Some(e), Some(0));
                                        }
                                    } else {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut op) = access.op_query.get_mut(id) {
                                                op.0 = op_str.into();
                                                access.op_changed_query.get_mut(id).unwrap().0 =
                                                    true;
                                                access.net_query.get_mut(id).unwrap().0 =
                                                    str_to_net(op_str);
                                                access.op_num_query.get_mut(id).unwrap().0 =
                                                    str_to_op_num(op_str);
                                            }
                                        }
                                        lt_to_open = (None, Some(0));
                                    }
                                }
                            }
                            Some("ord") | Some("order") => {
                                if let Some(s) = command.next() {
                                    if let Some(e) = str_to_id(s) {
                                        if let Ok(mut order) = access.order_query.get_mut(e) {
                                            if let Some(n) = command.next() {
                                                if let Ok(n) = n.parse::<f32>() {
                                                    if c0 == Some(":set") || c0 == Some("set") {
                                                        order.0 = n as usize;
                                                    } else {
                                                        order.0 = (order.0 as f32 + n) as usize;
                                                    }
                                                    access.order_change.send_default();
                                                }
                                            }
                                        }
                                    } else if let Ok(n) = s.parse::<f32>() {
                                        for id in access.selected_query.iter() {
                                            if let Ok(mut order) = access.order_query.get_mut(id) {
                                                if c0 == Some(":set") || c0 == Some("set") {
                                                    order.0 = n as usize;
                                                } else {
                                                    order.0 = (order.0 as f32 + n) as usize;
                                                }
                                                access.order_change.send_default();
                                            }
                                        }
                                    }
                                }
                            }
                            Some("arr") | Some("array") => {
                                let mut tmp = Vec::new();
                                let mut id = None;
                                if let Some(s) = command.next() {
                                    id = str_to_id(s);
                                    if let Ok(n) = parse_with_constants(s) {
                                        tmp.push(n);
                                    }
                                }
                                for n in command {
                                    if let Ok(n) = parse_with_constants(n) {
                                        tmp.push(n);
                                    }
                                }
                                if let Some(id) = id {
                                    if let Ok(mut arr) = access.arr_query.get_mut(id) {
                                        arr.0 = tmp;
                                        lt_to_open = (Some(id), Some(-13));
                                    }
                                } else {
                                    for id in access.selected_query.iter() {
                                        if let Ok(mut arr) = access.arr_query.get_mut(id) {
                                            arr.0.clone_from(&tmp);
                                        }
                                    }
                                    lt_to_open = (None, Some(-13));
                                }
                            }
                            Some("tar") | Some("targets") => {
                                let mut tmp = Vec::new();
                                for e in command {
                                    if let Some(e) = str_to_id(e) {
                                        tmp.push(e);
                                    }
                                }
                                // set the rest (cdr) as the targets of first (car)
                                if access.selected_query.is_empty() {
                                    if !tmp.is_empty() {
                                        let controller = tmp.remove(0);
                                        if let Ok(mut c) = access.targets_query.get_mut(controller)
                                        {
                                            c.0 = tmp;
                                            lt_to_open = (Some(controller), Some(-14));
                                        }
                                    }
                                } else {
                                    // all selected circles get the list of entities as targets
                                    for e in access.selected_query.iter() {
                                        if let Ok(mut c) = access.targets_query.get_mut(e) {
                                            c.0.clone_from(&tmp);
                                        }
                                    }
                                    lt_to_open = (None, Some(-14));
                                }
                            }
                            _ => {}
                        }
                    }
                    // target selected
                    Some(":tsel") => {
                        if let Some(s) = command.next() {
                            if let Some(e) = str_to_id(s) {
                                if let Ok(mut targets) = access.targets_query.get_mut(e) {
                                    targets.0.clear();
                                    for selected in access.selected_query.iter() {
                                        targets.0.push(selected);
                                    }
                                    lt_to_open = (Some(e), Some(-14));
                                }
                            }
                        }
                    }
                    Some(":reset_bloom") => {
                        *access.bloom.single_mut() = BloomSettings {
                            intensity: 0.5,
                            low_frequency_boost: 0.6,
                            low_frequency_boost_curvature: 0.4,
                            composite_mode: BloomCompositeMode::Additive,
                            ..default()
                        };
                    }
                    _ => {}
                }
                // open all white holes reading whatever changed
                if let (None, Some(lt)) = lt_to_open {
                    for id in access.selected_query.iter() {
                        if let Ok(holes) = holes_query.get(id) {
                            for hole in &holes.0 {
                                if let Ok(bh) = access.black_hole_query.get(*hole) {
                                    if let Ok(wh) = access.white_hole_query.get_mut(bh.wh) {
                                        if wh.link_types.0 == lt {
                                            access.white_hole_query.get_mut(bh.wh).unwrap().open =
                                                true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if let (Some(id), Some(lt)) = lt_to_open {
                    if let Ok(holes) = holes_query.get(id) {
                        for hole in &holes.0 {
                            if let Ok(bh) = access.black_hole_query.get(*hole) {
                                if let Ok(wh) = access.white_hole_query.get_mut(bh.wh) {
                                    if wh.link_types.0 == lt {
                                        access.white_hole_query.get_mut(bh.wh).unwrap().open = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            text.clear();
        }
        // key commands
        let mut command = text.as_str().split_ascii_whitespace();
        let c0 = command.next();
        match c0 {
            Some("d") => {
                next_mode.set(Mode::Draw);
                *text = "-- DRAW --".to_string();
            }
            Some("c") => {
                next_mode.set(Mode::Connect);
                *text = "-- CONNECT --".to_string();
            }
            Some("et") => {
                access.drag_modes.falsify();
                access.drag_modes.t = true;
                text.clear();
            }
            Some("er") => {
                access.drag_modes.falsify();
                access.drag_modes.r = true;
                text.clear();
            }
            Some("en") => {
                access.drag_modes.falsify();
                access.drag_modes.n = true;
                text.clear();
            }
            Some("eh") => {
                access.drag_modes.falsify();
                access.drag_modes.h = true;
                text.clear();
            }
            Some("es") => {
                access.drag_modes.falsify();
                access.drag_modes.s = true;
                text.clear();
            }
            Some("el") => {
                access.drag_modes.falsify();
                access.drag_modes.l = true;
                text.clear();
            }
            Some("ea") => {
                access.drag_modes.falsify();
                access.drag_modes.a = true;
                text.clear();
            }
            Some("eo") => {
                access.drag_modes.falsify();
                access.drag_modes.o = true;
                text.clear();
            }
            Some("ev") => {
                access.drag_modes.falsify();
                access.drag_modes.v = true;
                text.clear();
            }

            Some("ee") => {
                access.drag_modes.falsify();
                text.clear();
            }

            Some("Et") => {
                access.drag_modes.t = true;
                text.clear();
            }
            Some("Er") => {
                access.drag_modes.r = true;
                text.clear();
            }
            Some("En") => {
                access.drag_modes.n = true;
                text.clear();
            }
            Some("Eh") => {
                access.drag_modes.h = true;
                text.clear();
            }
            Some("Es") => {
                access.drag_modes.s = true;
                text.clear();
            }
            Some("El") => {
                access.drag_modes.l = true;
                text.clear();
            }
            Some("Ea") => {
                access.drag_modes.a = true;
                text.clear();
            }
            Some("Eo") => {
                access.drag_modes.o = true;
                text.clear();
            }
            Some("Ev") => {
                access.drag_modes.v = true;
                text.clear();
            }
            // toggle open white holes (selected)
            Some("ht") => {
                for id in access.selected_query.iter() {
                    if let Ok(mut wh) = access.white_hole_query.get_mut(id) {
                        wh.open = !wh.open;
                    }
                }
                text.clear();
            }
            // shortcuts
            Some("o") => {
                *text = ":set op ".to_string();
            }
            Some("l") => {
                *text = ":lt ".to_string();
            }
            // increment/decrement order
            Some("]") | Some("[") => {
                for id in access.selected_query.iter() {
                    if let Ok(mut order) = access.order_query.get_mut(id) {
                        if c0 == Some("]") {
                            order.0 += 1;
                        } else if order.0 > 0 {
                            order.0 -= 1;
                        }
                        access.order_change.send_default();
                    }
                }
                text.clear();
            }
            // increment/decrement link type
            Some("}") | Some("{") => {
                for id in access.selected_query.iter() {
                    if let Ok(mut wh) = access.white_hole_query.get_mut(id) {
                        if c0 == Some("}") {
                            wh.link_types.1 = wh.link_types.1.saturating_add(1);
                        } else {
                            wh.link_types.1 = wh.link_types.1.saturating_sub(1);
                        }
                        wh.open = true;
                    } else if let Ok(bh) = access.black_hole_query.get(id) {
                        let wh = &mut access.white_hole_query.get_mut(bh.wh).unwrap();
                        if c0 == Some("}") {
                            wh.link_types.0 = wh.link_types.0.saturating_add(1);
                        } else {
                            wh.link_types.0 = wh.link_types.0.saturating_sub(1);
                        }
                        wh.open = true;
                    }
                }
                text.clear();
            }
            // audio node inputs / outputs number / print info
            Some("ni") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(n) = access.net_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, n.0.inputs());
                    }
                }
                *text = format!(">INPUTS: {}", t);
            }
            Some("no") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(n) = access.net_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, n.0.outputs());
                    }
                }
                *text = format!(">OUTPUTS: {}", t);
            }
            Some("np") => {
                text.clear();
                for e in access.selected_query.iter() {
                    if let Ok(e) = access.op_query.get(e) {
                        *text += &format!("> {}\n", e.0);
                    }
                    if let Ok(e) = access.net_query.get(e) {
                        let mut net = e.0.clone();
                        *text += &net.display();
                        *text += &format!("Nodes          : {}\n", net.size());
                    }
                }
            }
            Some("ah") => {
                *text = ">HOSTS:\n".to_string();
                let hosts = cpal::platform::ALL_HOSTS;
                for (i, host) in hosts.iter().enumerate() {
                    *text += &format!("{}: {:?}\n", i, host);
                }
            }
            Some("ao") => {
                let hosts = cpal::platform::ALL_HOSTS;
                *text = ">OUTPUT DEVICES:\n".to_string();
                for (i, host) in hosts.iter().enumerate() {
                    *text += &format!("{}: {:?}:\n", i, host);
                    let devices =
                        cpal::platform::host_from_id(*host).unwrap().output_devices().unwrap();
                    for (j, device) in devices.enumerate() {
                        if let Ok(config) = device.default_input_config() {
                            let sr = config.sample_rate().0;
                            let b = config.buffer_size();
                            let b = if let cpal::SupportedBufferSize::Range { min, max } = b {
                                format!("min:{} max:{}", min, max)
                            } else {
                                String::from("unknown")
                            };
                            *text +=
                                &format!("    {}: {:?}  sr:{:?} b:[{}]\n", j, device.name(), sr, b);
                        } else {
                            *text += &format!("    {}: {:?}\n", j, device.name());
                        }
                    }
                }
            }
            Some("ai") => {
                let hosts = cpal::platform::ALL_HOSTS;
                *text = ">INPUT DEVICES:\n".to_string();
                for (i, host) in hosts.iter().enumerate() {
                    *text += &format!("{}: {:?}:\n", i, host);
                    let devices =
                        cpal::platform::host_from_id(*host).unwrap().input_devices().unwrap();
                    for (j, device) in devices.enumerate() {
                        if let Ok(config) = device.default_input_config() {
                            let sr = config.sample_rate().0;
                            let b = config.buffer_size();
                            let b = if let cpal::SupportedBufferSize::Range { min, max } = b {
                                format!("min:{} max:{}", min, max)
                            } else {
                                String::from("unknown")
                            };
                            *text +=
                                &format!("    {}: {:?}  sr:{:?} b:[{}]\n", j, device.name(), sr, b);
                        } else {
                            *text += &format!("    {}: {:?}\n", j, device.name());
                        }
                    }
                }
            }
            // inspect commands
            Some("ii") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    t = t + &format!("{}  ", e);
                }
                *text = format!(">ID: {}", t);
            }
            Some("in") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(n) = access.num_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, n.0);
                    }
                }
                *text = format!(">NUM: {}", t);
            }
            Some("ira") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let ra = access.trans_query.get(e).unwrap().scale.x;
                    t = t + &format!("[{}]{}  ", e, ra);
                }
                *text = format!(">RADIUS: {}", t);
            }
            Some("ix") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let x = access.trans_query.get(e).unwrap().translation.x;
                    t = t + &format!("[{}]{}  ", e, x);
                }
                *text = format!(">X: {}", t);
            }
            Some("iy") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let y = access.trans_query.get(e).unwrap().translation.y;
                    t = t + &format!("[{}]{}  ", e, y);
                }
                *text = format!(">Y: {}", t);
            }
            Some("iz") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let z = access.trans_query.get(e).unwrap().translation.z;
                    t = t + &format!("[{}]{}  ", e, z);
                }
                *text = format!(">Z: {}", t);
            }
            Some("ihu") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let h = access.col_query.get(e).unwrap().0.hue;
                    t = t + &format!("[{}]{}  ", e, h);
                }
                *text = format!(">HUE: {}", t);
            }
            Some("is") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let s = access.col_query.get(e).unwrap().0.saturation;
                    t = t + &format!("[{}]{}  ", e, s);
                }
                *text = format!(">SATURATION: {}", t);
            }
            Some("il") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let l = access.col_query.get(e).unwrap().0.lightness;
                    t = t + &format!("[{}]{}  ", e, l);
                }
                *text = format!(">LIGHTNESS: {}", t);
            }
            Some("ial") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let a = access.col_query.get(e).unwrap().0.alpha;
                    t = t + &format!("[{}]{}  ", e, a);
                }
                *text = format!(">ALPHA: {}", t);
            }
            Some("iv") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let v = access.vertices_query.get(e).unwrap().0;
                    t = t + &format!("[{}]{}  ", e, v);
                }
                *text = format!(">VERTICES: {}", t);
            }
            Some("iro") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    let ro = access.trans_query.get(e).unwrap().rotation.to_euler(EulerRot::XYZ).2;
                    t = t + &format!("[{}]{}  ", e, ro);
                }
                *text = format!(">ROTATION: {}", t);
            }
            Some("ior") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(or) = access.order_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, or.0);
                    }
                }
                *text = format!(">ORDER: {}", t);
            }
            Some("iop") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(op) = &access.op_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, op.0);
                    }
                }
                *text = format!(">OP: {}", t);
            }
            Some("iar") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(arr) = access.arr_query.get(e) {
                        t = t + &format!("[{}]{:?}  ", e, arr.0);
                    }
                }
                *text = format!(">ARRAY: {}", t);
            }
            Some("iho") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(holes) = holes_query.get(e) {
                        let mut a = String::new();
                        for h in &holes.0 {
                            a = a + &format!("    {}\n", h);
                        }
                        t = t + &format!("[{}]\n{}\n", e, a);
                    }
                }
                *text = format!(">HOLES:\n{}", t);
            }
            Some("it") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(targets) = access.targets_query.get(e) {
                        let mut a = String::new();
                        for t in &targets.0 {
                            a = a + &format!("{}, ", t);
                        }
                        t = t + &format!("[{}]: [{}]\n", e, a);
                    }
                }
                *text = format!(">TARGETS:\n{}", t);
            }
            Some("iL") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(wh) = access.white_hole_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, lt_to_string(wh.link_types.1));
                    }
                    if let Ok(bh) = access.black_hole_query.get(e) {
                        let wh = access.white_hole_query.get(bh.wh).unwrap();
                        t = t + &format!("[{}]{}  ", e, lt_to_string(wh.link_types.0));
                    }
                }
                *text = format!(">LINK TYPE: {}", t);
            }
            Some("iO") => {
                let mut t = String::new();
                for e in access.selected_query.iter() {
                    if let Ok(wh) = access.white_hole_query.get(e) {
                        t = t + &format!("[{}]{}  ", e, wh.open);
                    }
                }
                *text = format!(">WH_OPEN: {}", t);
            }
            Some("sa") => {
                for e in circle_query.iter() {
                    commands.entity(e).insert(Selected);
                }
                text.clear();
            }
            Some("sA") => {
                for e in access.selected_query.iter() {
                    commands.entity(e).remove::<Selected>();
                }
                text.clear();
            }
            Some("sc") => {
                for e in circle_query.iter() {
                    if access.order_query.contains(e) {
                        commands.entity(e).insert(Selected);
                    }
                }
                text.clear();
            }
            Some("sh") => {
                for e in circle_query.iter() {
                    if !access.order_query.contains(e) {
                        commands.entity(e).insert(Selected);
                    }
                }
                text.clear();
            }
            Some("sn") => {
                *text = format!(">entities selected: {}", access.selected_query.iter().len());
            }
            Some("sg") => {
                for e in access.selected_query.iter() {
                    if let Ok(holes) = holes_query.get(e) {
                        for hole in &holes.0 {
                            commands.entity(*hole).insert(Selected);
                        }
                    }
                }
                text.clear();
            }
            Some("sC") => {
                for e in access.selected_query.iter() {
                    if access.order_query.contains(e) {
                        commands.entity(e).remove::<Selected>();
                    }
                }
                text.clear();
            }
            Some("sH") => {
                for e in access.selected_query.iter() {
                    if !access.order_query.contains(e) {
                        commands.entity(e).remove::<Selected>();
                    }
                }
                text.clear();
            }
            Some("st") => {
                for e in access.selected_query.iter() {
                    if let Ok(targets) = access.targets_query.get(e) {
                        for t in &targets.0 {
                            commands.entity(*t).insert(Selected);
                        }
                        commands.entity(e).remove::<Selected>();
                    }
                }
                text.clear();
            }
            Some("sv") => {
                for e in access.selected_query.iter() {
                    commands.entity(e).remove::<Selected>();
                }
                for e in access.visible.single().get::<WithMesh2d>() {
                    if circle_query.contains(*e) {
                        commands.entity(*e).insert(Selected);
                    }
                }
                text.clear();
            }
            Some("sV") => {
                for e in access.visible.single().get::<WithMesh2d>() {
                    commands.entity(*e).remove::<Selected>();
                }
                text.clear();
            }
            // visibility
            Some("vv") => {
                *access.render_layers.single_mut() = RenderLayers::from_layers(&[0, 1, 2, 3, 4]);
                access.show_info_text.0 = true;
                text.clear();
            }
            Some("vc") => {
                let mut rl = access.render_layers.single_mut();
                *rl = if rl.intersects(&RenderLayers::layer(1)) {
                    rl.clone().without(1)
                } else {
                    rl.clone().with(1)
                };
                text.clear();
            }
            Some("vb") => {
                let mut rl = access.render_layers.single_mut();
                *rl = if rl.intersects(&RenderLayers::layer(2)) {
                    rl.clone().without(2)
                } else {
                    rl.clone().with(2)
                };
                text.clear();
            }
            Some("vw") => {
                let mut rl = access.render_layers.single_mut();
                *rl = if rl.intersects(&RenderLayers::layer(3)) {
                    rl.clone().without(3)
                } else {
                    rl.clone().with(3)
                };
                text.clear();
            }
            Some("va") => {
                let mut rl = access.render_layers.single_mut();
                *rl = if rl.intersects(&RenderLayers::layer(4)) {
                    rl.clone().without(4)
                } else {
                    rl.clone().with(4)
                };
                text.clear();
            }
            Some("vt") => {
                if access.show_info_text.0 {
                    for e in circle_query.iter() {
                        if let Ok((_, info_text)) = info_text_query.get(e) {
                            commands.entity(info_text.0).despawn();
                            commands.entity(e).remove::<InfoText>();
                        }
                    }
                }
                access.show_info_text.0 = !access.show_info_text.0;
                text.clear();
            }
            Some("vT") => {
                access.show_info_text.1 = !access.show_info_text.1;
                text.clear();
            }
            // copypasting
            Some("yy") | Some("\"+y") => {
                access.copy_event.send_default();
                text.clear();
            }
            Some("p") | Some("\"+p") => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(string) = access.clipboard.0.get_contents() {
                    let _ = access.paste_chan.0 .0.try_send(string);
                }
                #[cfg(target_arch = "wasm32")]
                if let Some(win) = web_sys::window() {
                    if let Some(clip) = win.navigator().clipboard() {
                        let sender = access.paste_chan.0 .0.clone();
                        // TODO(amy): can't we store this instead of forgetting?
                        let cb = wasm_bindgen::closure::Closure::new(
                            move |val: wasm_bindgen::JsValue| {
                                if let Some(string) = val.as_string() {
                                    let _ = sender.try_send(string);
                                }
                            },
                        );
                        let _ = clip.read_text().then(&cb);
                        // fuck it
                        std::mem::forget(cb);
                    }
                }
                text.clear();
            }
            Some(":delete") => {
                access.delete_event.send_default();
                text.clear();
            }
            Some(":help") | Some(":about") | Some("about") | Some("help") => {
                *text = format!(">see: {}", env!("CARGO_PKG_REPOSITORY"));
            }
            Some("version") | Some(":version") => {
                *text = format!(">quartz version: {}", &access.version.0);
            }
            Some("quartz") => {
                *text = String::from(
                    "> never gonna give you up
never gonna let you down
never gonna run around and desert you
never gonna make you cry
never gonna say goodbye
never gonna tell a lie and hurt you",
                );
            }
            Some("awa") => {
                let mut aw = "aw".repeat(100);
                aw.push('\n');
                *text = format!(">{}", aw.repeat(60));
            }
            _ => {}
        }
        text.truncate(12060);
    }
}

fn str_to_id(s: &str) -> Option<Entity> {
    let mut e = s.split('v');
    if let Some(i) = e.next() {
        if let Some(g) = e.next() {
            if let Ok(index) = i.parse::<u64>() {
                if let Ok(gen) = g.parse::<u64>() {
                    let bits = gen << 32 | index;
                    return Some(Entity::from_bits(bits));
                }
            }
        }
    }
    None
}
