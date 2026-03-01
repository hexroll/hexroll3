#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
    pbr_types::{STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT, PbrInput, pbr_input_new},
    pbr_functions as fns,
    pbr_bindings,
}
#import bevy_core_pipeline::tonemapping::tone_mapping

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var my_array_texture: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var my_array_texture_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> mixer: vec4<f32>;

@fragment
fn fragment(
    @builtin(front_facing) is_front: bool,
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    // Prepare a 'processed' StandardMaterial by sampling all textures to resolve
    // the material members
    var pbr_input: PbrInput = pbr_input_new();

    let day = textureSample(my_array_texture, my_array_texture_sampler, mesh.uv, 0);
    let night = textureSample(my_array_texture, my_array_texture_sampler, mesh.uv, 1);

    pbr_input.material.base_color = day;
    pbr_input.material.emissive = night;

    let double_sided = (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;

    let blend = mix(day, night, mixer.x);
    let output = vec4f(blend.xyz, blend.w * mixer.y);


//     pbr_input.frag_coord = mesh.position;
//     pbr_input.world_position = mesh.world_position;
//     pbr_input.world_normal = fns::prepare_world_normal(
//         mesh.world_normal,
//         double_sided,
//         is_front,
//     );
//
//     pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
//
//     pbr_input.N = normalize(pbr_input.world_normal);
//
// #ifdef VERTEX_TANGENTS
//     let Nt = textureSampleBias(pbr_bindings::normal_map_texture, pbr_bindings::normal_map_sampler, mesh.uv, view.mip_bias).rgb;
//     let TBN = fns::calculate_tbn_mikktspace(mesh.world_normal, mesh.world_tangent);
//     pbr_input.N = fns::apply_normal_mapping(
//         pbr_input.material.flags,
//         TBN,
//         double_sided,
//         is_front,
//         Nt,
//     );
// #endif
//
//     pbr_input.V = fns::calculate_view(mesh.world_position, pbr_input.is_orthographic);
//
    // return tone_mapping(fns::apply_pbr_lighting(pbr_input), view.color_grading);
    return output;
}
