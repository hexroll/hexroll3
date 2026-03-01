#import bevy_pbr::forward_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> base_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> layer_color: vec4<f32>;

fn sd_hexagon(p: vec2<f32>, r: f32) -> f32 {
    let k = vec3<f32>(-0.866025404, 0.5, 0.577350269);
    var z = abs(p);
    z -= 2.0 * min(dot(k.xy, z), 0.0) * k.xy;
    z -= vec2<f32>(clamp(z.x, -k.z * r, k.z * r), r);
    return length(z) * sign(z.y);
}


@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {

    let p = (2.0*mesh.uv - vec2<f32>(1.0,1.0));
    let si = 0.8;
    let d = sd_hexagon(p,si);
    let alpha = clamp(d*-7.0, 0.0, 1.0);

    let output = mix(layer_color, base_color, base_color.w);
    return vec4<f32>(output.xyz, max(output.w, 1.0-alpha));
}
