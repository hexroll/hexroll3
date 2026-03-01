#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> time: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> res: vec4<f32>;

fn pattern(x: f32, a: f32, b: f32,c: f32, d: f32, w: f32) -> f32{
    if (x < a) {
        return 1.0;
    } else if (x < b) {
        return 1.0 - (x - a)/w;
    } else if (x < c) {
        return 0.0;
    } else if (x < d) {
        return (x - c)/w;
    } else {
        return 1.0;
    }
}

fn interval(x: f32, l: f32, r: f32) -> f32 {
    let w: f32 = 1.0;
    let x2 = x - l * floor(x / l);
    let a = l * r / 2.0 - w / 2.0;
    let b = l * r / 2.0 + w / 2.0;
    let c = l - b;
    let d = l - a;
    let z = pattern(x2, a, b, c, d, w);
    return 1.0 - smoothstep(0.0, 1.0, z);
}

@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    let N = 13;
    var p = mesh.uv * 1000.0;
    p.x = p.x + time.x * -100.0;

    let y: i32 = i32(floor(f32(N) * p.y / mesh.uv.x));
    p.y = p.y % res.y/f32(N);
    p.y = p.y - 0.5*res.y/f32(N);

    let d = min(
      interval(p.x, 120.0, 0.5),
      interval(p.x, 90.0, 0.3)
    );

    let s = smoothstep(0.5, -0.5, abs(p.y) - 1.0);
    let d2 = d*s + 1.0*(1.0 - s);

    let ta = smoothstep(0.85,0.85, mesh.uv.y);
    let tb = smoothstep(0.15,0.15, mesh.uv.y);
    let tc = smoothstep(0.78,0.78, mesh.uv.y);
    let td = smoothstep(0.22,0.22, mesh.uv.y);

    let col = (1.0-(1.0-tb))* 1.0-td*(1.0-((1.0-ta)*tc));

    let background = material_color.xyz;
    let base = (background * 2.0) * (1.0-d2)*col;
    let multiplier = vec3<f32>(1.0,1.0,1.0)-base;
    let composite = background * multiplier + base;

    return vec4<f32>(composite, material_color.w);
}
