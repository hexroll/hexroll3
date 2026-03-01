#import bevy_pbr::{
    mesh_types::{MESH_FLAGS_SHADOW_RECEIVER_BIT, MESH_FLAGS_TRANSMITTED_SHADOW_RECEIVER_BIT},
    forward_io::VertexOutput,
    mesh_view_bindings::view,
    pbr_types::{STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT, PbrInput, pbr_input_new},
    pbr_functions as fns,
    pbr_bindings,
}
#import bevy_core_pipeline::tonemapping::tone_mapping

struct Controls {
    zoom_factor: f32,
    grid_mix: f32,
    blend: f32,
    scale: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> controls: Controls;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> bg_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> offset_color: vec4<f32>;

fn grad(z: vec2<i32>) -> vec2<f32> {
    let n = z.x + z.y * 11111;
    let n2 = (n << 13) ^ n;
    let n3 = (n * (n * n * 15731 + 789221) + 1376312589) >> 16;

    return vec2<f32>(cos(f32(n3)), sin(f32(n3)));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);

    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(dot(grad(i + vec2<i32>(0, 0)), f - vec2<f32>(0.0, 0.0)), dot(grad(i + vec2<i32>(1, 0)), f - vec2<f32>(1.0, 0.0)), u.x),
        mix(dot(grad(i + vec2<i32>(0, 1)), f - vec2<f32>(0.0, 1.0)), dot(grad(i + vec2<i32>(1, 1)), f - vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn contrast(v: f32, contrast_factor: f32) -> f32 {
  let a_contrasted = (v - 0.5) * contrast_factor + 0.5;
  return clamp(a_contrasted, 0.0, 1.0);
}

fn grid(fragCoord: vec2<f32>, space: f32, gridWidth: f32, strength: f32) -> f32 {
    let p = fragCoord - vec2<f32>(0.5, 0.5);
    let size = vec2<f32>(gridWidth);

    let a1 = p - fract(size / space) * space;
    let a2 = p + fract(size / space) * space;
    let a = a2 - a1;

    let g = min(a.x, a.y);
    let factor = 10.0;
    var tUv = fract(fragCoord * factor/5.0/space);
    tUv = abs(tUv - 0.5) * 2.0;

    var grid = max(tUv.x, tUv.y);
    let v = clamp(gridWidth+0.90, 0.91,0.99999);
    let diff = grid - v - 0.04;
    grid = smoothstep(1.0, v-controls.zoom_factor * 0.8, grid+diff);
    return clamp(grid, 1.0 - strength + controls.zoom_factor * 1.2, 1.0);
}

@fragment
fn fragment(
    @builtin(front_facing) is_front: bool,
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {

    let m = mat2x2<f32>(1.6, 1.2, -1.2, 1.6);

    let fragCoord = mesh.uv * 50.0 * controls.scale;

    var uv = fragCoord / 10.0;
    var noise_l_0 = 0.5000 * noise(uv);
    uv = m * uv;
    noise_l_0 += 0.2500 * noise(uv);
    uv = m * uv;
    noise_l_0 += 0.1250 * noise(uv);
    uv = m * uv;
    noise_l_0 += 0.0625 * noise(uv);
    uv = m * uv;
    noise_l_0 = 0.75 + 0.25 * noise_l_0;

    uv /= 20.0;
    var noise_l_1 = 0.5000 * noise(uv);
    uv = m * uv;
    noise_l_1 += 0.2500 * noise(uv);
    uv = m * uv;
    noise_l_1 += 0.1250 * noise(uv);
    uv = m * uv;
    noise_l_1 += 0.0625 * noise(uv);
    uv = m * uv;

    uv /= 160.0;
    var noise_l_2 = 0.5000 * noise(uv);
    uv = m * uv;
    noise_l_2 += 0.2500 * noise(uv);
    uv = m * uv;
    noise_l_2 += 0.1250 * noise(uv);
    uv = m * uv;
    noise_l_2 += 0.0625 * noise(uv);
    uv = m * uv;

    uv *= 1.0;
    var noise_l_3 = 0.5000 * noise(uv);
    uv = m * uv;
    noise_l_3 += 0.2500 * noise(uv);
    uv = m * uv;
    noise_l_3 += 0.1250 * noise(uv);
    uv = m * uv;
    noise_l_3 += 0.0625 * noise(uv);
    uv = m * uv;

    let noised = vec4<f32>(noise_l_0, noise_l_0, noise_l_0, 1.0);
    let z = 1.0;

    let col = bg_color.xyz; 


    let thick_str = contrast(noise_l_1, 0.9);
    //let both_grid_lines = grid(fragCoord, 25.0, noise_l_3*z, 1.00);// * grid(fragCoord, 50.0, noise_l_3*z,  1.0);
    let both_grid_lines =grid(fragCoord, 25.0, noise_l_3*z, 0.75) * grid(fragCoord, 50.0, noise_l_3*z,  1.0);
    let color = col * clamp(both_grid_lines + 0.6, 0.4, 1.0);

    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    let double_sided = (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;

    pbr_input.frag_coord = mesh.position;
    pbr_input.world_position = mesh.world_position;
    pbr_input.world_normal = fns::prepare_world_normal(
        mesh.world_normal,
        double_sided,
        is_front,
    );

    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;

    pbr_input.N = normalize(pbr_input.world_normal);
    pbr_input.flags |= MESH_FLAGS_SHADOW_RECEIVER_BIT;

#ifdef VERTEX_TANGENTS
    let Nt = textureSampleBias(pbr_bindings::normal_map_texture, pbr_bindings::normal_map_sampler, mesh.uv, view.mip_bias).rgb;
    let TBN = fns::calculate_tbn_mikktspace(mesh.world_normal, mesh.world_tangent);
    pbr_input.N = fns::apply_normal_mapping(
        pbr_input.material.flags,
        TBN,
        double_sided,
        is_front,
        Nt,
    );
#endif

    pbr_input.V = fns::calculate_view(mesh.world_position, pbr_input.is_orthographic);
    let step1 = tone_mapping(fns::apply_pbr_lighting(pbr_input), view.color_grading);
    let step2 = fns::main_pass_post_lighting_processing(pbr_input, step1);
    let step3 = step2 + offset_color;
    let blend = mix(step3.xyz, color.xyz, controls.grid_mix);
    return  vec4f(blend.xyz, controls.blend);
}

