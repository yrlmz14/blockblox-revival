struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) part_pos: vec3<f32>,
    @location(3) part_size: vec3<f32>,
    @location(4) part_color: vec3<f32>,
    @location(5) rot_x: vec3<f32>,
    @location(6) rot_y: vec3<f32>,
    @location(7) rot_z: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) local_pos: vec3<f32>,
    @location(3) part_size: vec3<f32>,
    @location(4) is_top: f32,
};

@vertex
fn vs_main(model: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let rot = mat3x3<f32>(inst.rot_x, inst.rot_y, inst.rot_z);
    let local = model.position * inst.part_size;
    let world_pos = rot * local + inst.part_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = inst.part_color;
    out.normal = normalize(rot * model.normal);
    out.local_pos = local;
    out.part_size = inst.part_size;
    out.is_top = select(0.0, 1.0, model.normal.y > 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.39, 0.87, 0.30));
    var shade = max(dot(normalize(in.normal), light_dir), 0.32);
    if (in.is_top > 0.5) {
        let sx = (in.local_pos.x + in.part_size.x * 0.5) - floor(in.local_pos.x + in.part_size.x * 0.5) - 0.5;
        let sz = (in.local_pos.z + in.part_size.z * 0.5) - floor(in.local_pos.z + in.part_size.z * 0.5) - 0.5;
        let r2 = sx * sx + sz * sz;
        if (r2 < 0.075) {
            shade = shade * 1.12;
        } else if (r2 < 0.115) {
            shade = shade * 0.86;
        }
    }
    return vec4<f32>(in.color * shade, 1.0);
}
