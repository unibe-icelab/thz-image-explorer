use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::MouseButton;
use bevy::pbr::MaterialPipeline;
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
use type_uuid::TypeUuid;

fn color_picker_widget(ui: &mut egui::Ui, color: &mut Color) -> egui::Response {
    let [r, g, b, a] = Srgba::from(*color).to_f32_array();
    let mut egui_color: egui::Rgba = egui::Rgba::from_srgba_unmultiplied(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    );
    let res = egui::widgets::color_picker::color_edit_button_rgba(
        ui,
        &mut egui_color,
        egui::color_picker::Alpha::Opaque,
    );
    let [r, g, b, a] = egui_color.to_srgba_unmultiplied();
    *color = Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    );
    res
}
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

#[derive(Asset, TypePath, AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "e3e3f3a1-4444-11ee-be56-0242ac120002"]
pub struct VolumeMaterial {
    #[texture(0, dimension = "3d")]
    #[sampler(1)]
    pub volume_texture: Handle<Image>,

    #[uniform(2)]
    pub opacity: f32,

    #[uniform(3)]
    pub volume_size: Vec3,
}

impl Material for VolumeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/volume_shader.wgsl".into() // path to the shader
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
fn generate_test_volume(width: u32, height: u32, depth: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((width * height * depth) as usize);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                // Generate the value (normalized between 0 and 1)
                let value = ((x + y + z) as f32) / (width + height + depth) as f32;
                // Map the f32 value to a u8 (0..255)
                let byte_value = (value * 255.0).min(255.0).max(0.0) as u8;
                data.push(byte_value);
            }
        }
    }
    data
}

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
        base_color: Color::srgba(0.8, 0.7, 0.6, 1.0), // Default opacity 1.0 (fully opaque)
        reflectance: 0.02,
        alpha_mode: AlphaMode::Blend,
        unlit: false,
        ..default()
    };
    let preview_material_handle = materials.add(default_material.clone());

    // The cube that will be rendered to the texture.
    commands
        .spawn((
            Mesh3d(cube_handle),
            MeshMaterial3d(preview_material_handle.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
        ))
        .insert(Plot3DObject)
        .insert(RenderLayers::layer(1));

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

#[derive(Resource)]
pub struct VolumeTextureResource {
    pub texture: Handle<Image>,
}

pub fn setup_volume_texture(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut volume_materials: ResMut<Assets<VolumeMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let width = 120;
    let height = 120;
    let depth = 2000;

    let width = 2;
    let height = 2;
    let depth = 2;

    // Generate the test volume data (this can be replaced with real volume data)
    let volume_data = generate_test_volume(width, height, depth);

    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: depth,
    };

    let volume_texture = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("texture"),
            size,
            dimension: TextureDimension::D3,
            format: TextureFormat::R8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        data: Some(volume_data),
        ..Default::default()
    };

    // Add the texture to Bevy's asset system
    let volume_texture_handle = images.add(volume_texture);

    // Insert the texture as a resource (so it can be used by shaders or materials)
    commands.insert_resource(VolumeTextureResource {
        texture: volume_texture_handle.clone(),
    });

    let volume_material_handle = volume_materials.add(VolumeMaterial {
        volume_texture: volume_texture_handle.clone(), // your 3D texture handle
        volume_size: Vec3::new(width as f32, height as f32, depth as f32),
        opacity: 1.0,
    });

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::from_length(1.0)))),
        MeshMaterial3d(volume_material_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        RenderLayers::layer(1),
    ));
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
    height: f32,
    ui: &mut Ui,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    preview_cube_query: Query<&MeshMaterial3d<StandardMaterial>, With<Plot3DObject>>,
) {
    ui.label("3D plot:");

    let available_size = egui::vec2(width.min(height), width.min(height));
    ui.label(format!("{} x {}", available_size.x, available_size.y));

    ui.vertical(|ui| {
        ui.allocate_ui(available_size, |ui| {
            // Show the texture in UI
            ui.image(egui::load::SizedTexture::new(
                *cube_preview_texture_id,
                available_size,
            ));

            let rect = ui.max_rect();

            let response = ui.interact(
                rect,
                egui::Id::from("sense"),
                egui::Sense::drag() | egui::Sense::hover(),
            );
            // Interaction for dragging
            if response.dragged() || response.hovered() {
                hovered.0 = true;
            } else {
                hovered.0 = false;
            }
        })
        .response;

        // Add a slider for opacity control
        ui.label("Opacity:");
        if let Ok(material_handle) = preview_cube_query.single() {
            let preview_material = materials.get_mut(material_handle).unwrap();

            let mut opacity = preview_material.base_color.alpha();
            // Create the slider
            ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0)); // Get the value from the slider
            preview_material.base_color.set_alpha(opacity);
        }
    });
}
