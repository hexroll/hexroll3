#import bevy_pbr::forward_io::VertexOutput
#import bevy_core_pipeline::oit::oit_draw
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> res: vec4<f32>;

fn hex(p: vec2<f32>) -> f32 {
    let s = vec2<f32>(1.7320508, 1.0);
    let p2 = abs(p);
    return max(dot(p2, s * 0.5), p2.y);
}

fn getHex(p: vec2<f32>) -> vec4<f32> {
    let s = vec2<f32>(1.7320508, 1.0);
    let hC: vec4<f32> = floor(vec4<f32>(p, p - vec2<f32>(1.0, 0.5)) / s.xyxy) + 0.5;
    let h: vec4<f32> = vec4<f32>(p - hC.xy * s, p - (hC.zw + 0.5) * s);
    if dot(h.xy, h.xy) < dot(h.zw, h.zw) {
         return vec4<f32>(h.xy, hC.xy);
    } else {
        return vec4<f32>(h.zw, hC.zw + 0.5);
    }
}

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let s = vec2<f32>(1.7320508, 1.0);
    let u: vec2<f32> = (mesh.uv.xy - res.xy * 0.5) / res.y;
    let scale: f32 = 9.62300;
    let h: vec4<f32> = getHex(u * scale + s.yx * 1.0 / 10.0);
    let eDist: f32 = hex(h.xy);
    let line_width = res.z; // 0.02 to 0.04
    let smoothing = res.w; // to 0.009
    let base_color = vec3<f32>(1.0,0.0,0.0);
    let col: vec3<f32> = mix(vec3<f32>(1.0), vec3<f32>(0.0), smoothstep(0.0, smoothing * scale, eDist - 0.5 + line_width));
    return vec4<f32>(material_color.xyz, (1.0-col.x)  * material_color.w);
}
