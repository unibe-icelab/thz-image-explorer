use crate::config::{send_latest_config, ConfigCommand, ThreadCommunication};
use crate::gui::application::{FileDialogState, THzImageExplorer, Tab};
use crate::gui::toggle_widget::toggle;
use bevy::render::camera::{ImageRenderTarget, RenderTarget};
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;
use bevy::winit::EventLoopProxyWrapper;
use bevy::{prelude::*, render::render_resource::*};
use bevy_egui::egui::{epaint, Ui};
use bevy_egui::{egui, EguiUserTextures};
use bevy_framepace::Limiter;
use bevy_panorbit_camera::{ActiveCameraData, PanOrbitCamera};
use bevy_voxel_plot::{InstanceData, InstanceMaterialData};
use ndarray::{Array1, Array3, ArrayView1, Axis};
use rayon::prelude::*;

#[derive(Resource)]
pub struct OpacityThreshold(pub f32);

#[derive(Resource, Clone)]
pub struct InstanceContainer(pub Vec<InstanceData>, pub f32, pub f32, pub f32);

#[derive(Deref, Resource)]
pub struct RenderImage(Handle<Image>);

#[derive(Resource, Default)]
pub struct CameraInputAllowed(pub bool);

#[derive(Resource)]
pub struct SceneVisibility(pub bool);

pub fn update_instance_buffer_system(
    visibility: Res<SceneVisibility>,
    mut query: Query<(&mut InstanceMaterialData, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    thread_communication: Res<ThreadCommunication>,
    opacity_threshold: Res<OpacityThreshold>,
    mut instances: ResMut<InstanceContainer>,
) {
    if !visibility.0 {
        return;
    }

    if let Ok(instances_temp) = thread_communication.voxel_plot_instances_lock.try_read() {
        instances.0 = instances_temp.0.clone();
        instances.1 = instances_temp.1;
        instances.2 = instances_temp.2;
        instances.3 = instances_temp.3;
    }

    let cube_width = instances.1;
    let cube_height = instances.2;
    let cube_depth = instances.3;

    let new_mesh = meshes.add(Cuboid::new(cube_width, cube_height, cube_depth));

    if let Ok((mut instance_data, mut mesh3d)) = query.single_mut() {
        instance_data.instances = instances.0.clone();
        mesh3d.0 = new_mesh;
        instance_data.instances.retain_mut(|instance| {
            if instance.color[3] >= opacity_threshold.0 {
                // Recalculate colormap for remaining instances
                // Normalize the opacity value relative to the threshold range
                let normalized_opacity =
                    (instance.color[3] - opacity_threshold.0) / (1.0 - opacity_threshold.0);
                let (r, g, b) = jet_colormap(normalized_opacity);

                // Update the color while keeping the original opacity
                instance.color =
                    LinearRgba::from(Color::srgba(r, g, b, normalized_opacity)).to_f32_array();

                true
            } else {
                false
            }
        });
    }
}

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
fn convolve1d(data: &ArrayView1<f32>, kernel: &[f32], contrast: f32) -> Array1<f32> {
    let radius = kernel.len() / 2;
    let mut output = Array1::<f32>::zeros(data.len());

    for (i, out) in output.iter_mut().enumerate() {
        let mut acc = 0.0;
        for (k, &coeff) in kernel.iter().enumerate() {
            let j = i as isize + k as isize - radius as isize;
            if (0..data.len() as isize).contains(&j) {
                acc += data[j as usize].powf(contrast) * coeff;
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
    scaling: usize,
    original_dimensions: (usize, usize, usize),
    thread_communication: &ThreadCommunication,
) -> (Vec<InstanceData>, f32, f32, f32) {
    let opacity_threshold = thread_communication.gui_settings.opacity_threshold;
    let contrast = thread_communication.gui_settings.contrast_3d;
    let sigma = thread_communication.gui_settings.kernel_sigma;
    let radius = thread_communication.gui_settings.kernel_radius;

    let grid_width = dataset.shape()[0];
    let grid_height = dataset.shape()[1];
    let grid_depth = dataset.shape()[2];

    // Keep base cube size constant for mesh creation
    let base_cube_size = 1.0 / 4.0;
    let cube_width = base_cube_size;
    let cube_height = base_cube_size;

    let c = 300_000_000.0;
    let cube_depth = base_cube_size / (time_span * c / 1.0e9 * 2.0);

    // Use original dimensions for consistent spacing
    let (orig_width, orig_height, orig_depth) = original_dimensions;

    // Calculate spacing to maintain overall plot size
    let spacing_width = (orig_width as f32 * cube_width) / grid_width as f32;
    let spacing_height = (orig_height as f32 * cube_height) / grid_height as f32;
    let spacing_depth = (orig_depth as f32 * cube_depth) / grid_depth as f32;

    let kernel = gaussian_kernel1d(sigma, radius);

    // Apply convolution
    dataset
        .axis_iter_mut(Axis(0))
        .into_par_iter()
        .for_each(|mut plane| {
            for mut line in plane.axis_iter_mut(Axis(0)) {
                line.mapv_inplace(|v| v.powi(2));
                let envelope = convolve1d(&line.view(), &kernel, contrast);
                line.assign(&envelope);
            }
        });

    // Filter lines by maximum value, then normalize remaining lines
    dataset
        .axis_iter_mut(Axis(0))
        .into_par_iter()
        .for_each(|mut plane| {
            for mut line in plane.axis_iter_mut(Axis(0)) {
                // Check if line's maximum value meets threshold
                let max = line.fold(f32::NEG_INFINITY, |a, &b| a.max(b));

                if max < opacity_threshold {
                    // Zero out the entire line if it doesn't meet threshold
                    line.fill(0.0);
                } else {
                    // Normalize the line if it meets threshold
                    let min = line.fold(f32::INFINITY, |a, &b| a.min(b));

                    if (max - min).abs() > 1e-6 {
                        line.mapv_inplace(|v| (v - min) / (max - min));
                    } else {
                        line.fill(0.0);
                    }
                }
            }
        });

    let dataset_slice = dataset.as_slice().unwrap();
    let mut final_opacities: Vec<f32> = Vec::from(dataset_slice);

    // Dynamic threshold for instance count limiting
    let max_instances = 2_000_000;
    let effective_threshold = if final_opacities.len() > max_instances {
        let (_, nth_element, _) = final_opacities
            .select_nth_unstable_by(max_instances - 1, |a, b| b.partial_cmp(a).unwrap());
        *nth_element
    } else {
        0.0 // Since we already filtered, use 0.0 as effective threshold
    };

    if let Ok(mut thrs) = thread_communication.opacity_threshold_lock.write() {
        *thrs = effective_threshold;
    }

    let total_instances = grid_width * grid_height * grid_depth;
    let mut instances = Vec::with_capacity(total_instances.min(max_instances));

    let total_plot_width = orig_width as f32 * base_cube_size;
    let total_plot_height = orig_height as f32 * base_cube_size;
    let total_plot_depth = orig_depth as f32 * cube_depth;

    let half_width = total_plot_width / 2.0;
    let half_height = total_plot_height / 2.0;
    let half_depth = total_plot_depth / 2.0;

    let cube_scale = scaling as f32;

    for x in 0..grid_width {
        for y in 0..grid_height {
            for z in 0..grid_depth {
                let flat_index = x * grid_height * grid_depth + y * grid_depth + z;
                let opacity = dataset_slice[flat_index];

                if opacity < effective_threshold {
                    continue;
                }

                let (r, g, b) =
                    jet_colormap((opacity - effective_threshold) / (1.0 - effective_threshold));

                let position = Vec3::new(
                    y as f32 * spacing_height - half_height,
                    half_width - x as f32 * spacing_width,
                    z as f32 * spacing_depth - half_depth,
                );

                instances.push(InstanceData {
                    pos_scale: [position.x, position.y, position.z, cube_scale],
                    color: LinearRgba::from(Color::srgba(r, g, b, opacity)).to_f32_array(),
                });
            }
        }
    }

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
    mut framepace_settings: ResMut<bevy_framepace::FramepaceSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut egui_user_textures: ResMut<EguiUserTextures>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut active_cam: ResMut<ActiveCameraData>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // limit the framerate
    framepace_settings.limiter = Limiter::from_framerate(30.0);

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
        // NoFrustumCulling,
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
            Transform::from_translation(Vec3::new(0.0, 0.1, 0.0)),
            Projection::Orthographic {
                0: OrthographicProjection {
                    scaling_mode: bevy::render::camera::ScalingMode::Fixed {
                        width: 512.0,
                        height: 512.0,
                    },
                    scale: 1.0,
                    ..OrthographicProjection::default_3d()
                },
            },
            PanOrbitCamera {
                allow_upside_down: true,
                pitch: Some(30.0_f32.to_radians()),
                yaw: Some(0.0_f32.to_radians()),
                ..default()
            },
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

// Animate the camera's position
pub fn animate(
    time: Res<Time>,
    mut pan_orbit_query: Query<&mut PanOrbitCamera>,
    thread_communication: Res<ThreadCommunication>,
    event_loop_proxy: Res<EventLoopProxyWrapper<bevy::winit::WakeUp>>,
) {
    if thread_communication.gui_settings.tab == Tab::ThreeD
        && thread_communication.gui_settings.animation_enabled
    {
        for mut pan_orbit in pan_orbit_query.iter_mut() {
            // Must set target values, not yaw/pitch directly
            pan_orbit.target_yaw += 7f32.to_radians() * time.delta_secs();

            // Force camera to update its transform
            pan_orbit.force_update = true;
        }
        let _ = event_loop_proxy.send_event(bevy::winit::WakeUp); // Wakes up the event loop
    }
}

pub fn three_dimensional_plot_ui(
    cube_preview_texture_id: &epaint::TextureId,
    width: f32,
    mut height: f32,
    ui: &mut Ui,
    opacity_threshold: &mut ResMut<OpacityThreshold>,
    cam_input: &mut ResMut<CameraInputAllowed>,
    thread_communication: &mut ResMut<ThreadCommunication>,
    explorer: &mut THzImageExplorer,
) {
    height -= 210.0;
    let available_size = egui::vec2(width.min(height), width.min(height));

    // need to do this to take it out of the next closure, we will put it back later
    let mut animation_enabled = thread_communication.gui_settings.animation_enabled;
    let mut kernel_radius = thread_communication.gui_settings.kernel_radius;
    let mut kernel_sigma = thread_communication.gui_settings.kernel_sigma;
    let mut constrast = thread_communication.gui_settings.contrast_3d;

    let mut minimum_threshold = 0.01;
    if let Ok(thrs) = thread_communication.opacity_threshold_lock.try_read() {
        minimum_threshold = *thrs;
    }

    ui.vertical(|ui| {
        ui.add_space(10.0);

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

        ui.style_mut().spacing.slider_width = 300.0;

        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut opacity_threshold.0, minimum_threshold..=1.0)
                    .text("Opacity Threshold"),
            );

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_opacity");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
            if info_button.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }

            egui::popup_below_widget(
                ui,
                popup_id,
                &info_button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("This sets the threshold below which instances are not rendered. Note that this might also change the color mapping.\"
                                \nThe value is relative to the maximum opacity of the dataset, \
                                so a value of 0.5 means that only instances with at least 50% opacity will be rendered.");
                },
            );
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {

            if ui
                .add(
                    egui::Slider::new(&mut constrast, 0.01..=5.0)
                        .step_by(0.01)
                        .text("Contrast"),
                )
                .changed()
            {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::Set3DContrast(constrast),
                );
            }

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_contrast");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
            if info_button.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }

            egui::popup_below_widget(
                ui,
                popup_id,
                &info_button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("This sets the contrast below of the 3D render. \"
                                \nIt adjusts the exponent of the intensity to opacity mapping, \
                                so a value of 2.0 means opacity = intensity ** 2.0.");
                },
            );
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {

            if ui
                .add(egui::Slider::new(&mut kernel_radius, 1..=50).text("Kernel Radius"))
                .changed()
            {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SetKernelRadius(kernel_radius),
                );
            }

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_radius");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
            if info_button.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }

            egui::popup_below_widget(
                ui,
                popup_id,
                &info_button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("This sets the radius of the 1D Kernel. \");
                                \nIt defines how many neighboring points are considered for the Gaussian convolution. \
                                \nA larger radius means more smoothing, but also more computation.");
                },
            );
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {

            if ui
                .add(egui::Slider::new(&mut kernel_sigma, 0.1..=50.0).text("Kernel Sigma"))
                .changed()
            {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SetKernelSigma(kernel_sigma),
                );
            }

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_sigma");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
            if info_button.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }

            egui::popup_below_widget(
                ui,
                popup_id,
                &info_button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("This sets the sigma of the 1D Kernel. \"
                                \nIt defines the standard deviation of the Gaussian function used for smoothing. \
                                \nA larger sigma means more smoothing, but also more computation.");
                },
            );
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Animate Camera:");
            ui.add(toggle(&mut animation_enabled));
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {

            if ui
                .button(egui::RichText::new(format!(
                    "{} Export VTU",
                    egui_phosphor::regular::FLOPPY_DISK
                )))
                .clicked()
            {
                explorer.file_dialog_state = FileDialogState::SaveToVTU;
            }

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_vtu");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
            if info_button.clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(popup_id));
            }

            egui::popup_below_widget(
                ui,
                popup_id,
                &info_button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("Export to a .vtu file (VTK Unstructured Grid File) for further 3D analysis. (e.g. ParaView)");
                },
            );
        });
    });

    thread_communication.gui_settings.opacity_threshold = opacity_threshold.0;

    // put it back
    thread_communication.gui_settings.animation_enabled = animation_enabled;
    thread_communication.gui_settings.kernel_radius = kernel_radius;
    thread_communication.gui_settings.kernel_sigma = kernel_sigma;
    thread_communication.gui_settings.contrast_3d = constrast;
}
