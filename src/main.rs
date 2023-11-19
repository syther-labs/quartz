//use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy::render::camera::ScalingMode;
use rand::prelude::random;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PanCamPlugin::default())
        .add_plugins(Shape2dPlugin::default())
        .insert_resource(ClearColor(Color::DARK_GRAY))
        .insert_resource(Msaa::Off)
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_circles, draw_circles))
        .add_systems(Update, toggle_pan)
        .run();
}

#[derive(Component)]
struct Pos {
    pos: Vec3
}

fn setup(mut commands: Commands) {
    // Spawn the camera
    commands.spawn((
        //Camera2dBundle::default(),
        Camera2dBundle {
            transform: Transform::from_translation(Vec3::Z), //push the camera "back" one unit
            projection: OrthographicProjection {
                scaling_mode: ScalingMode::AutoMin { //something to do with window size
                    min_width: 5.2 * 4.5,
                    min_height: 3.2 * 4.5,
                },
                ..default()
            },
        ..default()
        },
        PanCam {
            //limit zooming
            max_scale: Some(40.),
            min_scale: 0.25,
            ..default()
        },
    ));
}

fn spawn_circles(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    ) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        let (camera, camera_transform) = camera_query.single();
        let Some(cursor_position) = windows.single().cursor_position() else {
            return;
        };
        let Some(point) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
            return;
        };
        commands.spawn(    
            Pos {
                pos: point.extend(0.0)
            }
        );
    }
}

fn draw_circles(mut painter: ShapePainter, query: Query<&Pos>) {
    for pos in &query {
        painter.translate(Vec3::ZERO);
        painter.translate(pos.pos); //this is wrong, i think we're accumulating translation
        painter.color = Color::hsla(random::<f32>()*360., 100.0, 50.0, random::<f32>());
        painter.circle(5.);
    }
}

/*
fn draw_circles(painter: &mut ShapePainter, radius: f32) {
    painter.translate(-(Vec3::X + Vec3::NEG_Y) * f32::sqrt(radius) * 0.5);
    painter.color = Color::rgba(1.0, 0.0, 0.0, 0.5);
    painter.circle(radius);

    painter.rotate_z(-TAU / 3.0);
    painter.translate(Vec3::Y * radius * 1.2 + Vec3::Z * 0.0001);
    painter.color = Color::rgba(0.0, 1.0, 0.0, 0.5);
    painter.circle(radius);

    painter.rotate_z(-TAU / 3.0);
    painter.translate(Vec3::Y * radius * 1.2 + Vec3::Z * 0.0001);
    painter.color = Color::rgba(0.0, 0.0, 1.0, 0.5);
    painter.circle(radius);
}

fn draw_gallery(mut painter: ShapePainter) {
    let radius = 2.0;

    painter.reset();
    painter.translate(Vec3::X * radius * -4.0);
    painter.alpha_mode = AlphaMode::Add;
    draw_circles(&mut painter, radius);

    painter.reset();
    painter.alpha_mode = AlphaMode::Multiply;
    draw_circles(&mut painter, radius);

    painter.reset();
    painter.translate(Vec3::X * radius * 4.0);
    painter.alpha_mode = AlphaMode::Blend;
    draw_circles(&mut painter, radius);
}
*/

fn toggle_pan(mut query: Query<&mut PanCam>, keys: Res<Input<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) {
        for mut pancam in &mut query {
            pancam.enabled = !pancam.enabled;
        }
    }
}

