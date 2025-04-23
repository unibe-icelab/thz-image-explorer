use bevy::render::render_resource::*;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::egui::{epaint, Ui};
use rand::Rng;

pub fn setup_volume_texture(mut commands: Commands, mut voxel_meshes: ResMut<Assets<Mesh>>) {
    let width = 120;
    let height = 120;
    let depth = 2000;

    let width = 2;
    let height = 2;
    let depth = 2;

    // Create cube mesh once
    let cube = voxel_meshes.add(Mesh::from(Cuboid::new(0.1, 0.1, 0.1)));

    // Generate positions (e.g. 120 x 120 x 200)
    let nx = 120;
    let ny = 120;
    let nz = 200;

    let mut rng = rand::thread_rng();

    for _ in 0..(nx * ny * nz) {
        let x = rng.gen_range(-60.0..60.0);
        let y = rng.gen_range(-60.0..60.0);
        let z = rng.gen_range(-100.0..100.0);
        let a = rng.gen_range(0.0..1.0);

        // commands.spawn((
        //     Mesh3d(cube.clone()),
        //     MeshMaterial3d(volume_materials.add(VolumeMaterial {
        //         color: Vec4::new(0.5, 0.5, 0.1, a),
        //     })),
        //     Transform::from_translation(Vec3::new(x, y, z)),
        //     RenderLayers::layer(1),
        // ));
    }
}

pub fn three_dimensional_plot_ui(
    cube_preview_texture_id: &epaint::TextureId,
    width: f32,
    height: f32,
    ui: &mut Ui,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    // preview_cube_query: Query<&MeshMaterial3d<StandardMaterial>, With<Plot3DObject>>,
) {
    ui.label("3D plot:");

    let available_size = egui::vec2(width.min(height), width.min(height));

    ui.vertical(|ui| {
        ui.label(format!("{} x {}", available_size.x, available_size.y));

        // ui.allocate_ui(available_size, |ui| {
        //     // Show the texture in UI
        //     ui.image(egui::load::SizedTexture::new(
        //         *cube_preview_texture_id,
        //         available_size,
        //     ));
        //
        //     let rect = ui.max_rect();
        //
        //     let response = ui.interact(
        //         rect,
        //         egui::Id::from("sense"),
        //         egui::Sense::drag() | egui::Sense::hover(),
        //     );
        //     // Interaction for dragging
        //     if response.dragged() || response.hovered() {
        //         hovered.0 = true;
        //     } else {
        //         hovered.0 = false;
        //     }
        // })
        // .response;

        ui.image(egui::load::SizedTexture::new(
            *cube_preview_texture_id,
            available_size,
        ));

        // Add a slider for opacity control
        // ui.label("Opacity:");
        // if let Ok(material_handle) = preview_cube_query.single() {
        //     let preview_material = materials.get_mut(material_handle).unwrap();
        //
        //     let mut opacity = preview_material.base_color.alpha();
        //     // Create the slider
        //     ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0)); // Get the value from the slider
        //     preview_material.base_color.set_alpha(opacity);
        // }
    });
}
