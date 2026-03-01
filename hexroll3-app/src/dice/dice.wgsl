#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct EmissionSetup {
    emission_factor: f32,
    diffuse_to_emission_factor: f32,
    _padding1: f32,
    _padding2: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> dice_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var<uniform> numbers_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102)
var<uniform> emission_setup: EmissionSetup;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let mask_color = pbr_input.material.base_color;
    let mask_color_adjusted = step(mask_color, vec4<f32>(0.5, 0.5, 0.5, 0.5)); 
    let adjusted_color = mix(dice_color, numbers_color, mask_color_adjusted.x * mask_color_adjusted.y * mask_color_adjusted.z);
    pbr_input.material.base_color = adjusted_color;

    // mask_color can be white to black (1.0, 1.0, 1.0, 1.0) to (0.0,0.0,0.0,0.0)
    // Given target_color, set result_color to target_color if mask_color is white
    // or to white if mask_color is black with gradient color in between.
    let white = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    let emission = pbr_input.material.emissive;
    pbr_input.material.emissive = (emission + adjusted_color * emission_setup.diffuse_to_emission_factor) * emission_setup.emission_factor;


#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
#endif
    return out;
}
