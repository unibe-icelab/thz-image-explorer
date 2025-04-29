use crate::config::ThreadCommunication;
use bevy::render::camera::{ImageRenderTarget, RenderTarget};
use bevy::render::view::{NoFrustumCulling, RenderLayers};
use bevy::window::PrimaryWindow;
use bevy::{prelude::*, render::render_resource::*};
use bevy_egui::egui::{epaint, Ui};
use bevy_egui::{egui, EguiUserTextures};
use bevy_panorbit_camera::{ActiveCameraData, PanOrbitCamera};
use bevy_voxel_plot::{InstanceData, InstanceMaterialData};
use ndarray::{Array1, Array3, ArrayView1, Axis};
use rayon::prelude::*;
use std::time::Instant;

#[derive(Resource)]
pub struct OpacityThreshold(pub f32);

#[derive(Deref, Resource)]
pub struct RenderImage(Handle<Image>);

#[derive(Resource, Default)]
pub struct CameraInputAllowed(pub bool);

// Generate a 1D Gaussian kernel
fn gaussian_kernel1d(sigma: f32, radius: usize) -> Vec<f32> {
    let size = 2 * radius + 1;
    let mut kernel = Vec::with_capacity(size);

    let sigma2 = 2.0 * sigma * sigma;
    let mut sum = 0.0;

    for i in 0..size {
        let x = i as f32 - radius as f32;
        let value = (-x * x / sigma2).exp();
        sum += value;
        kernel.push(value);
    }

    // Normalize in-place
    for v in &mut kernel {
        *v /= sum;
    }

    kernel
}

// Apply 1D convolution (valid for edge-safe Gaussian)
fn convolve1d(data: &ArrayView1<f32>, kernel: &[f32]) -> Array1<f32> {
    let radius = kernel.len() / 2;
    let mut output = Array1::<f32>::zeros(data.len());

    for (i, out) in output.iter_mut().enumerate() {
        let mut acc = 0.0;
        for (k, &coeff) in kernel.iter().enumerate() {
            let j = i as isize + k as isize - radius as isize;
            if (0..data.len() as isize).contains(&j) {
                acc += data[j as usize] * coeff;
            }
        }
        *out = acc;
    }

    output
}

fn jet_colormap(value: f32) -> (f32, f32, f32) {
    let four_value = 4.0 * value;
    let r = (four_value - 1.5).clamp(0.0, 1.0);
    let g = (four_value - 0.5).clamp(0.0, 1.0) - (four_value - 2.5).clamp(0.0, 1.0);
    let b = 1.0 - (four_value - 1.5).clamp(0.0, 1.0);

    (r, g, b)
}

pub(crate) fn instance_from_data(
    time_span: f32,
    mut dataset: Array3<f32>,
) -> (Vec<InstanceData>, f32, f32, f32) {
    let timer = Instant::now();

    let grid_width = dataset.shape()[0];
    let grid_height = dataset.shape()[1];
    let grid_depth = dataset.shape()[2];

    let cube_width = 1.0 / 4.0;
    let cube_height = 1.0 / 4.0;
    let c = 300_000_000.0;
    let cube_depth = cube_width / (time_span * c / 1.0e9 * 2.0);

    // Precompute kernel once
    let kernel = gaussian_kernel1d(3.0, 9);

    // Step 1: Envelope (convolve and square)
    dataset
        .axis_iter_mut(Axis(0))
        .into_par_iter()
        .for_each(|mut plane| {
            for mut line in plane.axis_iter_mut(Axis(0)) {
                line.mapv_inplace(|v| v.powi(2)); // single squaring

                let envelope = convolve1d(&line.view(), &kernel);

                // Write envelope back
                line.assign(&envelope);
            }
        });

    println!("calculating envelope: {:?}", timer.elapsed());
    let timer = Instant::now();

    // Step 2: Normalize (min-max normalization along z)
    dataset
        .axis_iter_mut(Axis(0))
        .into_par_iter()
        .for_each(|mut plane| {
            for mut line in plane.axis_iter_mut(Axis(0)) {
                let min = line.fold(f32::INFINITY, |a, &b| a.min(b));
                let max = line.fold(f32::NEG_INFINITY, |a, &b| a.max(b));

                if (max - min).abs() > 1e-6 {
                    line.mapv_inplace(|v| (v - min) / (max - min));
                } else {
                    line.fill(0.0);
                }
            }
        });

    println!("normalizing: {:?}", timer.elapsed());
    let timer = Instant::now();

    let total_instances = grid_width * grid_height * grid_depth;
    let mut instances = Vec::with_capacity(total_instances);

    // Precalculate
    let half_width = (grid_width as f32 * cube_width) / 2.0;
    let half_height = (grid_height as f32 * cube_height) / 2.0;
    let half_depth = (grid_depth as f32 * cube_depth) / 2.0;

    let dataset_slice = dataset.as_slice().unwrap(); // SAFER: unwrap once, not every time

    for x in 0..grid_width {
        for y in 0..grid_height {
            for z in 0..grid_depth {
                let flat_index = x * grid_height * grid_depth + y * grid_depth + z;
                let mut opacity = dataset_slice[flat_index];
                opacity = opacity * opacity; // opacity.powf(2.0)

                let (r, g, b) = jet_colormap(opacity);

                let position = Vec3::new(
                    x as f32 * cube_width - half_width,
                    y as f32 * cube_height - half_height,
                    z as f32 * cube_depth - half_depth,
                );

                instances.push(InstanceData {
                    pos_scale: [position.x, position.y, position.z, 1.0],
                    color: LinearRgba::from(Color::srgba(r, g, b, opacity)).to_f32_array(),
                });
            }
        }
    }
    println!("pushing instances: {:?}", timer.elapsed());

    (instances, cube_width, cube_height, cube_depth)
}

pub fn set_enable_camera_controls_system(
    cam_input: Res<CameraInputAllowed>,
    mut pan_orbit_query: Query<&mut PanOrbitCamera>,
) {
    for mut pan_orbit in pan_orbit_query.iter_mut() {
        pan_orbit.enabled = cam_input.0;
    }
}

pub fn setup(
    mut meshes: ResMut<Assets<Mesh>>,
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut active_cam: ResMut<ActiveCameraData>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let (instances, cube_width, cube_height, cube_depth) = (vec![], 1.0, 1.0, 1.0);

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(cube_width, cube_height, cube_depth))),
        InstanceMaterialData { instances },
        // NOTE: Frustum culling is done based on the Aabb of the Mesh and the GlobalTransform.
        // As the cube is at the origin, if its Aabb moves outside the view frustum, all the
        // instanced cubes will be culled.
        // The InstanceMaterialData contains the 'GlobalTransform' information for this custom
        // instancing, and that is not taken into account with the built-in frustum culling.
        // We must disable the built-in frustum culling by adding the `NoFrustumCulling` marker
        // component to avoid incorrect culling.
        NoFrustumCulling,
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 2.0, // Increase this to wash out shadows
        affects_lightmapped_meshes: false,
    });

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
    commands.insert_resource(RenderImage(image_handle.clone()));

    // This specifies the layer used for the first pass, which will be attached to the first pass camera and cube.
    let first_pass_layer = RenderLayers::layer(0);

    let pan_orbit_id = commands
        .spawn((
            Camera {
                // render before the "main pass" camera
                clear_color: ClearColorConfig::Custom(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                order: -1,
                target: RenderTarget::Image(ImageRenderTarget::from(image_handle.clone())),
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
        .single()
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

pub fn three_dimensional_plot_ui(
    meshes: &mut ResMut<Assets<Mesh>>,
    cube_preview_texture_id: &epaint::TextureId,
    width: f32,
    mut height: f32,
    ui: &mut Ui,
    query: &mut Query<(&mut InstanceMaterialData, &mut Mesh3d)>,
    opacity_threshold: &mut ResMut<OpacityThreshold>,
    cam_input: &mut ResMut<CameraInputAllowed>,
    thread_communication: &mut ResMut<ThreadCommunication>,
) {
    height -= 100.0;
    let available_size = egui::vec2(width.min(height), width.min(height));

    if let Ok(read_guard) = thread_communication.voxel_plot_instances_lock.read() {
        let (instances, cube_width, cube_height, cube_depth) = read_guard.clone();

        let new_mesh = meshes.add(Cuboid::new(cube_width, cube_height, cube_depth));

        ui.vertical(|ui| {
            ui.label("3D Voxel Plot");

            if ui.button("Refresh").clicked() {
                // Update existing entity
                if let Ok((mut instance_data, mut mesh3d)) = query.single_mut() {
                    instance_data.instances = instances.clone();
                    mesh3d.0 = new_mesh.clone();

                    instance_data
                        .instances
                        .retain(|instance| instance.color[3] >= opacity_threshold.0);
                } else {
                    println!("No existing entity found to update.");
                }
            }

            ui.allocate_ui(available_size, |ui| {
                ui.image(egui::load::SizedTexture::new(
                    *cube_preview_texture_id,
                    available_size,
                ));

                let rect = ui.max_rect();

                let response = ui.interact(
                    rect,
                    egui::Id::new("sense"),
                    egui::Sense::drag() | egui::Sense::hover(),
                );

                if response.dragged() || response.hovered() {
                    cam_input.0 = true;
                } else {
                    cam_input.0 = false;
                }
            });

            ui.label("Opacity:");

            if ui
                .add(
                    egui::Slider::new(&mut opacity_threshold.0, 0.01..=1.0)
                        .text("Opacity Threshold"),
                )
                .changed()
            {
                if let Ok((mut instance_data, mut mesh3d)) = query.single_mut() {
                    instance_data.instances = instances;
                    mesh3d.0 = new_mesh;
                    instance_data
                        .instances
                        .retain(|instance| instance.color[3] >= opacity_threshold.0);
                }
            }
        });
    }
}
