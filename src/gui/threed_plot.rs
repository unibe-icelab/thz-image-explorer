use bevy::input::mouse::MouseButton;
use bevy::render::render_resource::*;
use bevy::{
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
};
use bevy_egui::egui::{epaint, Ui};
use bevy_egui::{egui, EguiUserTextures};

#[derive(Component)]
pub struct Plot3DCameraController {
    pub radius: f32,
    pub theta: f32, // horizontal rotation
    pub phi: f32,   // vertical rotation
    pub focus: Vec3,
    pub sensitivity: f32,
    pub zoom_speed: f32,
}

#[derive(Resource, Default)]
pub struct Plot3DHovered(pub bool);

#[derive(Component)]
pub struct Plot3DCamera;

#[derive(Resource)]
pub struct Plot3DRender {
    pub texture_id: egui::TextureId,
    pub image_handle: Handle<Image>,
}

#[derive(Deref, Resource)]
pub struct CubePreviewImage(Handle<Image>);

#[derive(Component)]
pub struct Plot3DObject;

pub fn setup_plot_3d_render(
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
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

    let cube_handle = meshes.add(Cuboid::new(4.0, 4.0, 4.0));
    let default_material = StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        reflectance: 0.02,
        unlit: false,
        ..default()
    };
    let preview_material_handle = materials.add(default_material.clone());

    // This specifies the layer used for the preview pass, which will be attached to the preview pass camera and cube.
    let preview_pass_layer = RenderLayers::layer(1);

    // The cube that will be rendered to the texture.
    commands
        .spawn((
            Mesh3d(cube_handle),
            MeshMaterial3d(preview_material_handle),
            Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
        ))
        .insert(Plot3DObject)
        .insert(preview_pass_layer.clone());

    // The same light is reused for both passes,
    // you can specify different lights for preview and main pass by setting appropriate RenderLayers.
    commands
        .spawn((
            PointLight::default(),
            Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)),
        ))
        .insert(RenderLayers::default().with(1));

    commands.spawn((
        Camera3d::default(), // marker component
        Camera {
            order: -1,
            target: RenderTarget::Image(image_handle.clone().into()),
            clear_color: ClearColorConfig::Custom(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 15.0)).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(), // Required in 0.13 for spatial hierarchy
        RenderLayers::layer(1),
        Plot3DCamera,
        Plot3DCameraController {
            radius: 15.0,
            theta: 0.0,
            phi: std::f32::consts::FRAC_PI_4,
            focus: Vec3::ZERO,
            sensitivity: 0.01,
            zoom_speed: 0.05,
        },
    ));
    commands.insert_resource(Plot3DHovered(false));
}

pub fn plot_3d_camera_controller(
    mut motion_evr: EventReader<bevy::input::mouse::MouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut scroll_evr: EventReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    hovered: Res<Plot3DHovered>, // custom resource, see below
    mut query: Query<(&mut Transform, &mut Plot3DCameraController), With<Plot3DCamera>>,
) {
    let window = if let Ok(w) = windows.get_single() {
        w
    } else {
        return;
    };

    if !hovered.0 {
        return; // Don't rotate if mouse isn't over UI plot area
    }

    let mut delta = Vec2::ZERO;
    for ev in motion_evr.read() {
        delta += ev.delta;
    }

    let scroll: f32 = scroll_evr.read().map(|e| e.y).sum();

    for (mut transform, mut controller) in query.iter_mut() {
        // Zoom
        controller.radius -= scroll * controller.zoom_speed;
        controller.radius = controller.radius.clamp(2.0, 100.0);

        if buttons.pressed(MouseButton::Left) {
            controller.theta += delta.x * controller.sensitivity;
            controller.phi -= delta.y * controller.sensitivity;
            controller.phi = controller.phi.clamp(0.1, std::f32::consts::PI - 0.1);
        }

        // Update position
        let sin_phi = controller.phi.sin();
        let x = controller.radius * controller.theta.cos() * sin_phi;
        let y = controller.radius * controller.phi.cos();
        let z = controller.radius * controller.theta.sin() * sin_phi;

        let eye = controller.focus + Vec3::new(x, y, z);
        *transform = Transform::from_translation(eye).looking_at(controller.focus, Vec3::Y);
    }
}

pub fn three_dimensional_plot_ui(
    hovered: &mut ResMut<Plot3DHovered>,
    cube_preview_texture_id: &epaint::TextureId,
    width: f32,
    heigth: f32,
    ui: &mut Ui,
) {
    ui.label("3D plot:");

    let available_size = egui::vec2(width.min(heigth), heigth.min(width));

    ui.allocate_ui(available_size, |ui| {
        // Show the texture in UI
        ui.image(egui::load::SizedTexture::new(
            *cube_preview_texture_id,
            available_size,
        ));

        let rect = ui.max_rect();

        if ui
            .interact(rect, ui.id(), egui::Sense::drag() | egui::Sense::hover())
            .dragged()
        {
            hovered.0 = true;
        }
    })
    .response;
}
