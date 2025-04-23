use bevy::render::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;
use bevy::{core_pipeline::tonemapping::Tonemapping, prelude::*, reflect::TypePath, render};
use bevy_common_assets::json::JsonAssetPlugin;
use bevy_egui::EguiUserTextures;
use bevy_panorbit_camera::{ActiveCameraData, PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_vector_shapes::prelude::*;
use serde::Deserialize;
use type_uuid::TypeUuid;

#[derive(Deserialize, TypeUuid, TypePath, Debug)]
#[uuid = "413be529-7234-7423-7421-4b8b380a2c46"]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Deserialize, TypeUuid, TypePath, Debug)]
#[uuid = "413be529-6233-6345-7534-4b8b380a2c46"]
pub struct Location {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Deserialize, TypeUuid, TypePath, Debug)]
#[uuid = "413be529-7234-1753-5413-4b8b380a2c46"]
pub struct Point {
    pub color: Color,
    pub highlight: bool, // TODO: implement highlight animation
    pub location: Location,
    pub size: f32,
}

#[derive(Asset, Deserialize, TypeUuid, TypePath, Debug)]
#[uuid = "413be529-bfeb-41b3-9db0-4b8b380a2c46"]
pub struct Points {
    points: Vec<Point>,
}

#[derive(Deref, Resource)]
pub struct CubePreviewImage(Handle<Image>);

#[derive(Resource)]
struct PointsHandle(Handle<Points>);

pub fn setup(
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut active_cam: ResMut<ActiveCameraData>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
        ..default()
    };

    // This is the texture that will be rendered to.
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };

    // fill image.data with zeroes
    image.resize(size);

    let image_handle = images.add(image);
    egui_user_textures.add_image(image_handle.clone());
    commands.insert_resource(CubePreviewImage(image_handle.clone()));

    // This specifies the layer used for the first pass, which will be attached to the first pass camera and cube.
    let first_pass_layer = RenderLayers::layer(0);

    commands.insert_resource(PointsHandle(
        asset_server.load("tsne_p100_i1000.points.json"),
    ));

    let pan_orbit_id = commands
        .spawn((
            Camera {
                // render before the "main pass" camera
                clear_color: ClearColorConfig::Custom(bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.0)),
                order: -1,
                target: RenderTarget::Image(image_handle.clone()),
                ..default()
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 15.0)).looking_at(Vec3::ZERO, Vec3::Y),
            PanOrbitCamera::default(),
            first_pass_layer,
        ))
        .id();

    // Set up manual override of PanOrbitCamera. Note that this must run after PanOrbitCameraPlugin
    // is added, otherwise ActiveCameraData will be overwritten.
    // Note: you probably want to update the `viewport_size` and `window_size` whenever they change,
    // I haven't done this here for simplicity.
    let primary_window = windows
        .get_single()
        .expect("There is only ever one primary window");
    active_cam.set_if_neq(ActiveCameraData {
        // Set the entity to the entity ID of the camera you want to control. In this case, it's
        // the inner (first pass) cube that is rendered to the texture/image.
        entity: Some(pan_orbit_id),
        // What you set these values to will depend on your use case, but generally you want the
        // viewport size to match the size of the render target (image, viewport), and the window
        // size to match the size of the window that you are interacting with.
        viewport_size: Some(Vec2::new(size.width as f32, size.height as f32)),
        window_size: Some(Vec2::new(primary_window.width(), primary_window.height())),
        // Setting manual to true ensures PanOrbitCameraPlugin will not overwrite this resource
        manual: true,
    });
}

pub fn draw_points(point_groups: ResMut<Assets<Points>>, mut shapes: ShapePainter) {
    for point_group in point_groups.iter() {
        for point in point_group.1.points.iter() {
            shapes.set_translation(Vec3::new(
                point.location.x,
                point.location.y,
                point.location.z,
            ));
            shapes.color = bevy::color::Color::srgba(
                point.color.r,
                point.color.g,
                point.color.b,
                point.color.a,
            );
            shapes.circle(point.size);
        }
    }
}
