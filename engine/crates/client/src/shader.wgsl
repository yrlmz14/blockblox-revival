struct FrameUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,      // xyz = normalized sun dir, w = brightness
    sky_top: vec4<f32>,      // rgb = zenith, w = time
    sky_bottom: vec4<f32>,   // rgb = horizon, w = fog_end
    ambient: vec4<f32>,      // rgb = ambient, w = shadow_enabled
    hud_stick: vec4<f32>,    // xy = stick center (ndc 0..1), zw = knob offset (px-normalized)
    hud_screen: vec4<f32>,   // x = width, y = height, z = stick_active, w = jump_pressed
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

@group(1) @binding(0)
var shadow_map: texture_depth_2d;
@group(1) @binding(1)
var shadow_sampler: sampler_comparison;

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

// -------------------- 1. SHADOW DEPTH PASS --------------------

@vertex
fn vs_shadow(model: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    let rot = mat3x3<f32>(inst.rot_x, inst.rot_y, inst.rot_z);
    let world_pos = rot * (model.position * inst.part_size) + inst.part_pos;
    return frame.light_view_proj * vec4<f32>(world_pos, 1.0);
}

// -------------------- 2. PROCEDURAL CLASSIC SKYBOX --------------------

struct SkyOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOutput {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: SkyOutput;
    out.clip_pos = vec4<f32>(pos[vi], 0.9999, 1.0);
    out.ndc = pos[vi];
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm_clouds(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        v += a * value_noise(q);
        q = q * 2.05 + vec2<f32>(13.7, 7.3);
        a *= 0.5;
    }
    return v;
}

@fragment
fn fs_sky(in: SkyOutput) -> @location(0) vec4<f32> {
    let far_clip = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let world_far = far_clip.xyz / far_clip.w;
    let dir = normalize(world_far - frame.camera_pos.xyz);

    let elev = clamp(dir.y, 0.0, 1.0);
    let horizon_t = pow(1.0 - elev, 2.2);
    var col = mix(frame.sky_top.rgb, frame.sky_bottom.rgb, horizon_t);

    // Below horizon: soft classic lower-sky blue/gray
    if (dir.y < 0.0) {
        let below = clamp(-dir.y * 3.0, 0.0, 1.0);
        col = mix(frame.sky_bottom.rgb, frame.sky_bottom.rgb * 0.65, below);
    }

    // Classic Roblox fluffy clouds in upper sky
    if (dir.y > 0.03) {
        let cloud_uv = dir.xz / (dir.y + 0.18) * 1.15 + vec2<f32>(frame.sky_top.w * 0.012, 0.0);
        let n = fbm_clouds(cloud_uv);
        let cloud_mask = smoothstep(0.52, 0.78, n) * smoothstep(0.03, 0.22, dir.y);
        let cloud_col = mix(vec3<f32>(0.90, 0.94, 0.99), vec3<f32>(1.0, 0.99, 0.96), 0.6);
        col = mix(col, cloud_col, cloud_mask * 0.72);
    }

    // Sun disc + warm halo
    let sun_dir = normalize(frame.sun_dir.xyz);
    let sun_dot = max(dot(dir, sun_dir), 0.0);
    let sun_glow = pow(sun_dot, 12.0) * 0.28 + pow(sun_dot, 140.0) * 0.55;
    let sun_disc = smoothstep(0.9978, 0.9990, sun_dot);
    col += vec3<f32>(1.0, 0.95, 0.80) * sun_glow + vec3<f32>(1.0, 0.98, 0.90) * sun_disc * 1.2;

    return vec4<f32>(col, 1.0);
}

// -------------------- 3. CLASSIC ROBLOX PART SHADER --------------------

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
    @location(3) local_pos: vec3<f32>,
    @location(4) part_size: vec3<f32>,
    @location(5) local_normal: vec3<f32>,
};

@vertex
fn vs_main(model: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let rot = mat3x3<f32>(inst.rot_x, inst.rot_y, inst.rot_z);
    let local = model.position * inst.part_size;
    let world_pos = rot * local + inst.part_pos;
    out.clip_position = frame.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = inst.part_color;
    out.normal = normalize(rot * model.normal);
    out.world_pos = world_pos;
    out.local_pos = local;
    out.part_size = inst.part_size;
    out.local_normal = model.normal;
    return out;
}

fn sample_shadow(world_pos: vec3<f32>, n: vec3<f32>, sun_dir: vec3<f32>) -> f32 {
    let ndotl = max(dot(n, sun_dir), 0.0);
    let bias_offset = n * (0.08 + 0.14 * (1.0 - ndotl));
    let light_clip = frame.light_view_proj * vec4<f32>(world_pos + bias_offset, 1.0);
    let ndc = light_clip.xyz / light_clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let depth = ndc.z;

    let in_bounds = frame.ambient.w >= 0.5
        && uv.x >= 0.01 && uv.x <= 0.99
        && uv.y >= 0.01 && uv.y <= 0.99
        && depth >= 0.0 && depth <= 1.0;

    let clamped_uv = clamp(uv, vec2<f32>(0.002), vec2<f32>(0.998));
    let cmp_depth = clamp(depth - 0.0015, 0.0, 1.0);

    let texel = 1.0 / 2048.0;
    var vis = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let off = vec2<f32>(f32(x), f32(y)) * texel;
            vis += textureSampleCompareLevel(shadow_map, shadow_sampler, clamped_uv + off, cmp_depth);
        }
    }
    return select(1.0, vis / 9.0, in_bounds);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let sun_dir = normalize(frame.sun_dir.xyz);
    let view_vec = frame.camera_pos.xyz - in.world_pos;
    let dist = length(view_vec);
    let view_dir = view_vec / max(dist, 0.0001);

    let ndotl = max(dot(n, sun_dir), 0.0);
    let shadow = sample_shadow(in.world_pos, n, sun_dir);

    // Hemisphere ambient (sky top vs warm ground bounce)
    let hemi = n.y * 0.5 + 0.5;
    let sky_bounce = mix(frame.ambient.rgb * 0.78, frame.sky_top.rgb * 0.35 + frame.ambient.rgb * 0.65, hemi);

    // Classic Roblox plastic lighting
    let sun_light = vec3<f32>(1.0, 0.97, 0.91) * (ndotl * shadow * 0.78 * frame.sun_dir.w);
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(n, half_vec), 0.0), 36.0) * shadow * 0.18 * step(0.01, ndotl);

    var surface_mul = 1.0;

    // Top face: classic 1x1 Roblox studs
    if (in.local_normal.y > 0.85) {
        let sx = (in.local_pos.x + in.part_size.x * 0.5) - floor(in.local_pos.x + in.part_size.x * 0.5) - 0.5;
        let sz = (in.local_pos.z + in.part_size.z * 0.5) - floor(in.local_pos.z + in.part_size.z * 0.5) - 0.5;
        let r = sqrt(sx * sx + sz * sz);
        // Stud cap + raised rim shading
        if (r < 0.25) {
            surface_mul = 1.09 + 0.05 * clamp((sx + sz) * 2.0, -1.0, 1.0);
        } else if (r < 0.32) {
            surface_mul = 0.84;
        }
    } else if (in.local_normal.y < -0.85) {
        // Bottom face: classic Roblox inlets
        let sx = (in.local_pos.x + in.part_size.x * 0.5) - floor(in.local_pos.x + in.part_size.x * 0.5) - 0.5;
        let sz = (in.local_pos.z + in.part_size.z * 0.5) - floor(in.local_pos.z + in.part_size.z * 0.5) - 0.5;
        let r = sqrt(sx * sx + sz * sz);
        if (r < 0.24) {
            surface_mul = 0.86;
        } else if (r < 0.31) {
            surface_mul = 1.06;
        }
    }

    // Subtle brick bevel along part edges
    let half_sz = max(in.part_size * 0.5, vec3<f32>(0.01));
    let edge_dist = half_sz - abs(in.local_pos);
    let min_edge = min(
        select(edge_dist.x, 99.0, abs(in.local_normal.x) > 0.8),
        min(
            select(edge_dist.y, 99.0, abs(in.local_normal.y) > 0.8),
            select(edge_dist.z, 99.0, abs(in.local_normal.z) > 0.8),
        ),
    );
    let bevel = smoothstep(0.0, 0.06, min_edge);
    surface_mul *= mix(0.88, 1.0, bevel);

    var lit = in.color * (sky_bounce + sun_light) * surface_mul + vec3<f32>(spec);

    // Gentle distance fog for depth
    let fog_t = clamp((dist - 140.0) / max(frame.sky_bottom.w - 140.0, 80.0), 0.0, 0.82);
    lit = mix(lit, frame.sky_bottom.rgb, fog_t);

    return vec4<f32>(lit, 1.0);
}

// -------------------- 4. CLASSIC MOBILE HUD OVERLAY --------------------

struct HudOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_hud(@builtin(vertex_index) vi: u32) -> HudOutput {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: HudOutput;
    out.clip_pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = vec2<f32>(pos[vi].x * 0.5 + 0.5, 0.5 - pos[vi].y * 0.5);
    return out;
}

@fragment
fn fs_hud(in: HudOutput) -> @location(0) vec4<f32> {
    let w = max(frame.hud_screen.x, 1.0);
    let h = max(frame.hud_screen.y, 1.0);
    let px = in.uv * vec2<f32>(w, h);
    let short_side = min(w, h);

    // Virtual Thumbstick (bottom-left)
    let stick_r = short_side * 0.115;
    let stick_center = vec2<f32>(
        frame.hud_stick.x * w,
        frame.hud_stick.y * h,
    );
    let knob_center = stick_center + frame.hud_stick.zw * stick_r;
    let d_stick = length(px - stick_center);
    let d_knob = length(px - knob_center);

    var out_col = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Outer thumbstick ring
    if (d_stick < stick_r + 3.0) {
        let fill_a = select(0.18, 0.28, frame.hud_screen.z > 0.5);
        let ring = smoothstep(stick_r - 3.0, stick_r - 1.0, d_stick) * (1.0 - smoothstep(stick_r, stick_r + 2.5, d_stick));
        let fill = 1.0 - smoothstep(stick_r - 1.5, stick_r + 0.5, d_stick);
        out_col = vec4<f32>(0.12, 0.15, 0.20, fill * fill_a) + vec4<f32>(1.0, 1.0, 1.0, ring * 0.55);
    }

    // Inner thumbstick knob
    let knob_r = stick_r * 0.42;
    if (d_knob < knob_r + 2.0) {
        let k = 1.0 - smoothstep(knob_r - 1.5, knob_r + 1.0, d_knob);
        let ka = select(0.45, 0.78, frame.hud_screen.z > 0.5) * k;
        out_col = vec4<f32>(mix(out_col.rgb, vec3<f32>(1.0, 1.0, 1.0), ka), max(out_col.a, ka));
    }

    // Classic Roblox Jump Button (bottom-right)
    let jump_r = short_side * 0.088;
    let jump_center = vec2<f32>(w - short_side * 0.15, h - short_side * 0.16);
    let d_jump = length(px - jump_center);
    if (d_jump < jump_r + 3.0) {
        let pressed = frame.hud_screen.w > 0.5;
        let fill = 1.0 - smoothstep(jump_r - 1.5, jump_r + 0.5, d_jump);
        let ring = smoothstep(jump_r - 3.0, jump_r - 1.0, d_jump) * (1.0 - smoothstep(jump_r, jump_r + 2.5, d_jump));
        let bg_a = select(0.24, 0.48, pressed) * fill;
        out_col = vec4<f32>(0.10, 0.14, 0.20, bg_a) + vec4<f32>(1.0, 1.0, 1.0, ring * 0.65);

        // Crisp up-arrow inside Jump button
        let lp = (px - jump_center) / jump_r;
        let shaft = step(abs(lp.x), 0.11) * step(-0.12, lp.y) * step(lp.y, 0.42);
        let head = step(lp.y, 0.02) * step(-0.46, lp.y) * step(abs(lp.x), (lp.y + 0.46) * 0.85);
        let arrow = clamp(shaft + head, 0.0, 1.0);
        if (arrow > 0.5) {
            out_col = vec4<f32>(1.0, 1.0, 1.0, 0.88);
        }
    }

    if (out_col.a < 0.01) {
        discard;
    }
    return out_col;
}
