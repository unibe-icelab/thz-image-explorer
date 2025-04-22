@group(0) @binding(0)
var volume_texture: texture_3d<f32>;  // 3D texture binding

@group(0) @binding(1)
var volume_sampler: sampler;  // Sampler for texture

// Add volume size as a uniform
@group(0) @binding(2)
var volume_size: vec3<f32>;  // Store texture size (width, height, depth)

@fragment
fn fragment_shader(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Normalize the coordinates based on volume size
    let uv = frag_coord.xy / volume_size.xy;  // Normalize with texture width/height
    let depth = frag_coord.z / volume_size.z;  // Normalize with texture depth

    // Sample the texture with normalized coordinates
    let voxel_value = textureSample(volume_texture, volume_sampler, vec3<f32>(uv, depth));

    // Return the final color
    return vec4<f32>(1.0, 1.0, 1.0, voxel_value);
}