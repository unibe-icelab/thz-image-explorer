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

    commands
        .spawn((
            Camera3d::default(),
            Camera {
                // render before the "main pass" camera
                order: -1,
                target: RenderTarget::Image(image_handle.into()),
                clear_color: ClearColorConfig::Custom(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                ..default()
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 15.0))
                .looking_at(Vec3::default(), Vec3::Y),
        ))
        .insert(preview_pass_layer);

}

pub fn three_dimensional_plot_ui(
    cube_preview_texture_id: &epaint::TextureId,
    ui: &mut Ui,
) {
    ui.label("plot: ");

    // Show the texture in UI
    ui.image(egui::load::SizedTexture::new(
        *cube_preview_texture_id,
        egui::vec2(500., 500.),
    ));
}
