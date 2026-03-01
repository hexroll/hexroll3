#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> res: vec4<f32>;

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    var p = mesh.uv;
    p.x = p.x * 10.0 + 0.25;
    p.y = (p.y - 0.5) * 0.75;

    let smoothv = res.x;
    let v = vec2<f32>(floor(p.x + 0.5), 0.0);
 	let d = distance(p, v);
    let c = smoothstep(d-smoothv, d+smoothv, 0.2);
    let base = vec4<f32>(c) * material_color;

    return vec4<f32>(base);
}
