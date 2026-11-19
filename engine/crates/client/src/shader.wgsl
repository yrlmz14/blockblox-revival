struct FrameUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,      // xyz = normalized sun dir, w = brightness
    sky_top: vec4<f32>,      // rgb = zenith (sRGB), w = time
    sky_bottom: vec4<f32>,   // rgb = horizon (sRGB), w = fog_end
    ambient: vec4<f32>,      // rgb = ambient (sRGB), w = shadow_enabled
    hud_stick: vec4<f32>,    // xy = stick center (ndc 0..1), zw = knob offset (px-normalized)
    hud_screen: vec4<f32>,   // x = width, y = height, z = stick_active, w = jump_pressed
    hud_core: vec4<f32>,     // x = health (0..1), y = tool_count (0..6), z = active_tool, w = show_touch
    fog_params: vec4<f32>,   // x = fog_start, y = fog_end, z = has_cubemap (0/1), w = unused
    hud_tools_a: vec4<f32>,  // per-slot ToolIconKind for slots 0, 1, 2, 3
    hud_tools_b: vec4<f32>,  // per-slot ToolIconKind for slots 4, 5, unused, unused
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

@group(1) @binding(0)
var shadow_map: texture_depth_2d;
@group(1) @binding(1)
var shadow_sampler: sampler_comparison;

@group(1) @binding(2)
var skybox_cube: texture_cube<f32>;
@group(1) @binding(3)
var skybox_sampler: sampler;

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
    @location(8) mat_params: vec4<f32>, // x = material, y = surfaces, z = decal, w = alpha
};

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
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

fn fbm(p: vec2<f32>) -> f32 {
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

// -------------------- 1. SHADOW DEPTH PASS --------------------

@vertex
fn vs_shadow(model: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    let rot = mat3x3<f32>(inst.rot_x, inst.rot_y, inst.rot_z);
    let world_pos = rot * (model.position * inst.part_size) + inst.part_pos;
    return frame.light_view_proj * vec4<f32>(world_pos, 1.0);
}

// -------------------- 2. CLASSIC ROBLOX SKYBOX (GPU CUBEMAP OR PROCEDURAL) --------------------

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

@fragment
fn fs_sky(in: SkyOutput) -> @location(0) vec4<f32> {
    let far_clip = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let world_far = far_clip.xyz / far_clip.w;
    let dir = normalize(world_far - frame.camera_pos.xyz);

    // Sample .rbxl cubemap texture uniformly (when loaded)
    let cube_col = textureSample(skybox_cube, skybox_sampler, vec3<f32>(dir.x, dir.y, dir.z)).rgb;

    let sky_top_lin = srgb_to_linear(frame.sky_top.rgb);
    let sky_bot_lin = srgb_to_linear(frame.sky_bottom.rgb);

    let elev = clamp(dir.y, 0.0, 1.0);
    let horizon_t = pow(1.0 - elev, 2.3);
    var proc_col = mix(sky_top_lin, sky_bot_lin, horizon_t);

    if (dir.y < 0.0) {
        let below = clamp(-dir.y * 3.2, 0.0, 1.0);
        proc_col = mix(sky_bot_lin, sky_bot_lin * 0.45, below);
    }

    // Layered classic Roblox fluffy clouds
    if (dir.y > 0.02) {
        let cloud_uv = dir.xz / (dir.y + 0.22) * 1.1 + vec2<f32>(frame.sky_top.w * 0.010, 0.0);
        let n1 = fbm(cloud_uv);
        let n2 = fbm(cloud_uv * 2.1 + vec2<f32>(4.2, -2.8));
        let n = n1 * 0.72 + n2 * 0.28;
        let cloud_mask = smoothstep(0.50, 0.76, n) * smoothstep(0.02, 0.20, dir.y);
        let shade = smoothstep(0.48, 0.80, n1);
        let cloud_col = mix(vec3<f32>(0.68, 0.76, 0.88), vec3<f32>(1.0, 0.99, 0.96), shade);
        proc_col = mix(proc_col, cloud_col, cloud_mask * 0.85);
    }

    // Sun disc + warm atmospheric halo
    let sun_dir = normalize(frame.sun_dir.xyz);
    let sun_dot = max(dot(dir, sun_dir), 0.0);
    let sun_glow = pow(sun_dot, 10.0) * 0.18 + pow(sun_dot, 120.0) * 0.45;
    let sun_disc = smoothstep(0.9980, 0.9991, sun_dot);
    proc_col += vec3<f32>(1.0, 0.92, 0.72) * sun_glow + vec3<f32>(1.0, 0.98, 0.90) * sun_disc * 1.6;

    let col = select(proc_col, cube_col, frame.fog_params.z > 0.5);
    return vec4<f32>(col, 1.0);
}

// -------------------- 3. CLASSIC ROBLOX PART & CHARACTER SHADER --------------------

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
    @location(3) local_pos: vec3<f32>,
    @location(4) part_size: vec3<f32>,
    @location(5) local_normal: vec3<f32>,
    @location(6) mat_params: vec4<f32>,
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
    out.mat_params = inst.mat_params;
    return out;
}

fn sample_shadow(world_pos: vec3<f32>, n: vec3<f32>, sun_dir: vec3<f32>) -> f32 {
    let ndotl = max(dot(n, sun_dir), 0.0);
    let bias_offset = n * (0.06 + 0.12 * (1.0 - ndotl));
    let light_clip = frame.light_view_proj * vec4<f32>(world_pos + bias_offset, 1.0);
    let ndc = light_clip.xyz / light_clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let depth = ndc.z;

    let in_bounds = frame.ambient.w >= 0.5
        && uv.x >= 0.01 && uv.x <= 0.99
        && uv.y >= 0.01 && uv.y <= 0.99
        && depth >= 0.0 && depth <= 1.0;

    let clamped_uv = clamp(uv, vec2<f32>(0.002), vec2<f32>(0.998));
    let cmp_depth = clamp(depth - 0.0014, 0.0, 1.0);

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

// Triplanar UV helper for procedural Roblox materials
fn face_uv(local_pos: vec3<f32>, local_n: vec3<f32>) -> vec2<f32> {
    let an = abs(local_n);
    if (an.y >= an.x && an.y >= an.z) {
        return local_pos.xz;
    } else if (an.x >= an.z) {
        return local_pos.zy;
    }
    return local_pos.xy;
}

// Classic Roblox Face SDF (front -Z of Head mesh)
fn eval_classic_face(uv: vec2<f32>) -> vec4<f32> {
    // uv in [-0.5, 0.5] across front of head (+x right, +y up)
    // Flip x so face looks outward
    let p = vec2<f32>(-uv.x, uv.y);

    // Classic Roblox glossy oval eyes
    let eye_l = vec2<f32>(-0.145, 0.09);
    let eye_r = vec2<f32>(0.145, 0.09);
    let el = (p - eye_l) / vec2<f32>(0.052, 0.072);
    let er = (p - eye_r) / vec2<f32>(0.052, 0.072);
    let d_eye = min(length(el), length(er));
    let eye_mask = 1.0 - smoothstep(0.86, 1.04, d_eye);

    // Subtle white eye catchlight
    let gl = length((p - (eye_l + vec2<f32>(-0.015, 0.024))) / vec2<f32>(0.016, 0.018));
    let gr = length((p - (eye_r + vec2<f32>(-0.015, 0.024))) / vec2<f32>(0.016, 0.018));
    let glint = (1.0 - smoothstep(0.7, 1.1, min(gl, gr))) * eye_mask;

    // Classic curved Roblox smile arc
    let sc = vec2<f32>(0.0, 0.055);
    let sp = p - sc;
    let smile_r = 0.215;
    let ang = atan2(sp.y, sp.x);
    let in_arc = step(-2.42, ang) * step(ang, -0.72);
    let arc_dist = abs(length(sp) - smile_r);

    // Rounded end caps for the smile
    let cap_l = length(sp - vec2<f32>(cos(-2.42), sin(-2.42)) * smile_r);
    let cap_r = length(sp - vec2<f32>(cos(-0.72), sin(-0.72)) * smile_r);
    let d_smile = select(min(cap_l, cap_r), min(arc_dist, min(cap_l, cap_r)), in_arc > 0.5);
    let smile_mask = 1.0 - smoothstep(0.018, 0.028, d_smile);

    let ink = clamp(eye_mask + smile_mask, 0.0, 1.0);
    let face_col = mix(vec3<f32>(0.03, 0.03, 0.04), vec3<f32>(0.92, 0.94, 0.98), glint * 0.85);
    return vec4<f32>(face_col, ink);
}

// Classic Roblox SpawnLocation sunburst decal (+Y face)
fn eval_spawn_decal(uv: vec2<f32>) -> vec4<f32> {
    // uv in [-0.5, 0.5] across top of SpawnLocation
    let r = length(uv);
    // Outer white ring
    let outer_ring = (1.0 - smoothstep(0.39, 0.41, r)) * smoothstep(0.33, 0.35, r);
    // Inner circle
    let inner_circle = 1.0 - smoothstep(0.11, 0.13, r);
    // 8 radiating rays
    let ang = atan2(uv.y, uv.x);
    let ray_wave = abs(cos(ang * 4.0));
    let rays = smoothstep(0.82, 0.90, ray_wave) * smoothstep(0.15, 0.18, r) * (1.0 - smoothstep(0.30, 0.33, r));
    let mask = clamp(outer_ring + inner_circle + rays, 0.0, 1.0);
    return vec4<f32>(vec3<f32>(0.96, 0.97, 0.99), mask * 0.92);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_n = normalize(in.normal);
    let local_n = normalize(in.local_normal);
    let sun_dir = normalize(frame.sun_dir.xyz);
    let view_vec = frame.camera_pos.xyz - in.world_pos;
    let dist = length(view_vec);
    let view_dir = view_vec / max(dist, 0.0001);

    let mat_id = i32(round(in.mat_params.x));
    let surf_packed = i32(round(in.mat_params.y));
    let top_surf = surf_packed % 16;
    let bot_surf = surf_packed / 16;
    let decal_id = i32(round(in.mat_params.z));
    let alpha = in.mat_params.w;

    var albedo = srgb_to_linear(in.color);
    var spec_strength = 0.22;
    var spec_power = 42.0;
    var metallic = 0.0;
    var emissive = vec3<f32>(0.0);
    var bump_shade = 1.0;

    let uv = face_uv(in.local_pos, local_n);
    // Smooth distance fade for high-frequency micro-patterns (computed uniformly via dist)
    let stud_fade = 1.0 - smoothstep(55.0, 145.0, dist);
    let detail_fade = 1.0 - smoothstep(45.0, 130.0, dist);

    // ---- Procedural Roblox Materials ----
    if (mat_id == 1) {
        // SmoothPlastic (glossy, crisp highlights, used on R6 avatars & smooth builds)
        spec_strength = 0.26;
        spec_power = 54.0;
    } else if (mat_id == 2) {
        // Wood (flowing wood grain rings)
        let grain_coord = vec2<f32>(uv.x * 1.4, uv.y * 0.28);
        let warp = fbm(grain_coord * 2.2) * 2.4;
        let rings = sin((uv.x * 5.5 + warp) * 3.14159) * 0.5 + 0.5;
        let fine = value_noise(vec2<f32>(uv.x * 18.0, uv.y * 2.5));
        bump_shade *= mix(1.0, 0.80 + 0.28 * rings + 0.08 * fine, detail_fade);
        spec_strength = 0.09;
        spec_power = 18.0;
    } else if (mat_id == 3) {
        // WoodPlanks (horizontal planks with seams + wood grain)
        let plank_row = floor(uv.y * 1.25);
        let plank_y = fract(uv.y * 1.25);
        let plank_x = fract(uv.x * 0.25 + hash21(vec2<f32>(plank_row, 3.1)) * 0.7);
        let seam_y = smoothstep(0.0, 0.05, plank_y) * smoothstep(0.0, 0.05, 1.0 - plank_y);
        let seam_x = smoothstep(0.0, 0.015, plank_x) * smoothstep(0.0, 0.015, 1.0 - plank_x);
        let plank_var = 0.88 + 0.24 * hash21(vec2<f32>(plank_row, floor(uv.x * 0.25)));
        let grain = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 14.0)) * 0.14;
        bump_shade *= mix(1.0, (0.62 + 0.38 * seam_y * seam_x) * plank_var + grain, detail_fade);
        spec_strength = 0.10;
        spec_power = 20.0;
    } else if (mat_id == 4) {
        // Brick (classic Roblox staggered brick pattern with recessed mortar)
        let row = floor(uv.y * 1.2);
        let bx = fract(uv.x * 0.5 + (row % 2.0) * 0.5);
        let by = fract(uv.y * 1.2);
        let mortar_x = smoothstep(0.0, 0.05, bx) * smoothstep(0.0, 0.05, 1.0 - bx);
        let mortar_y = smoothstep(0.0, 0.09, by) * smoothstep(0.0, 0.09, 1.0 - by);
        let mortar = mortar_x * mortar_y;
        let brick_id = vec2<f32>(floor(uv.x * 0.5 + (row % 2.0) * 0.5), row);
        let brick_var = 0.84 + 0.30 * hash21(brick_id);
        let grit = value_noise(uv * 10.0) * 0.10;
        let brick_col = albedo * (brick_var + grit);
        let mortar_col = vec3<f32>(0.36, 0.35, 0.33);
        albedo = mix(albedo, mix(mortar_col, brick_col, mortar), detail_fade);
        bump_shade *= mix(1.0, 0.78 + 0.22 * mortar, detail_fade);
        spec_strength = 0.06;
        spec_power = 14.0;
    } else if (mat_id == 5 || mat_id == 6) {
        // Concrete / Slate (multi-scale pitted stone noise)
        let n1 = fbm(uv * 1.6);
        let n2 = value_noise(uv * 9.0);
        let rock = 0.78 + 0.28 * n1 + 0.12 * n2;
        bump_shade *= mix(1.0, rock, detail_fade);
        spec_strength = 0.08;
        spec_power = 16.0;
    } else if (mat_id == 7) {
        // Cobblestone
        let gx = fract(uv.x * 0.8);
        let gy = fract(uv.y * 0.8);
        let edge = smoothstep(0.0, 0.12, gx) * smoothstep(0.0, 0.12, 1.0 - gx)
                 * smoothstep(0.0, 0.12, gy) * smoothstep(0.0, 0.12, 1.0 - gy);
        let cvar = 0.75 + 0.35 * hash21(floor(uv * 0.8));
        bump_shade *= mix(1.0, (0.55 + 0.45 * edge) * cvar, detail_fade);
        spec_strength = 0.10;
        spec_power = 20.0;
    } else if (mat_id == 8) {
        // Metal (brushed metallic sheen)
        let brush = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 24.0));
        bump_shade *= mix(1.0, 0.90 + 0.18 * brush, detail_fade);
        spec_strength = 0.65;
        spec_power = 72.0;
        metallic = 0.65;
    } else if (mat_id == 9) {
        // DiamondPlate (classic Roblox industrial diamond tread)
        let dp = fract(uv * 1.2) - 0.5;
        let rot_a = abs(dp.x + dp.y) + abs(dp.x - dp.y) * 2.4;
        let rot_b = abs(dp.x - dp.y) + abs(dp.x + dp.y) * 2.4;
        let cell = floor(uv * 1.2);
        let alt = (cell.x + cell.y) % 2.0;
        let d = select(rot_b, rot_a, alt < 0.5);
        let bump = 1.0 - smoothstep(0.32, 0.44, d);
        bump_shade *= mix(1.0, 0.86 + 0.28 * bump, detail_fade);
        spec_strength = 0.55 + 0.25 * bump;
        spec_power = 64.0;
        metallic = 0.55;
    } else if (mat_id == 10) {
        // Grass (classic Roblox organic grass blades & patches)
        let patch_n = fbm(uv * 0.75);
        let blades = value_noise(uv * 8.5);
        let grass_tint = mix(albedo * 0.78, albedo * 1.18, patch_n);
        albedo = mix(albedo, grass_tint, detail_fade * 0.85);
        bump_shade *= mix(1.0, 0.84 + 0.28 * blades, detail_fade);
        spec_strength = 0.05;
        spec_power = 12.0;
    } else if (mat_id == 11) {
        // Sand
        let grains = value_noise(uv * 16.0);
        let ripple = sin(uv.x * 2.2 + fbm(uv * 0.8) * 2.0) * 0.06;
        bump_shade *= mix(1.0, 0.90 + 0.14 * grains + ripple, detail_fade);
        spec_strength = 0.14;
        spec_power = 28.0;
    } else if (mat_id == 12) {
        // Fabric (subtle woven crosshatch)
        let weave = (sin(uv.x * 26.0) * sin(uv.y * 26.0)) * 0.08;
        bump_shade *= mix(1.0, 0.94 + weave, detail_fade);
        spec_strength = 0.04;
        spec_power = 10.0;
    } else if (mat_id == 13 || mat_id == 14) {
        // Ice / Glass (glossy Fresnel sky reflection)
        let frost = fbm(uv * 2.5);
        bump_shade *= mix(1.0, 0.94 + 0.12 * frost, detail_fade);
        spec_strength = 0.80;
        spec_power = 96.0;
        metallic = 0.35;
    } else if (mat_id == 15) {
        // Neon (self-lit bright glow!)
        emissive = albedo * 1.45;
        spec_strength = 0.0;
    }

    // ---- Classic Roblox 3D Studs & Inlets (Anti-Aliased) ----
    if (local_n.y > 0.85 && top_surf == 1) {
        let sx = fract(in.local_pos.x + in.part_size.x * 0.5) - 0.5;
        let sz = fract(in.local_pos.z + in.part_size.z * 0.5) - 0.5;
        let r = sqrt(sx * sx + sz * sz);
        let aa = 0.025 + dist * 0.0018;
        let cap = 1.0 - smoothstep(0.255 - aa, 0.255 + aa, r);
        let rim = smoothstep(0.235 - aa, 0.255 + aa, r) * (1.0 - smoothstep(0.305 - aa, 0.325 + aa, r));
        let bevel_light = clamp((sx * sun_dir.x + sz * sun_dir.z) * 2.8, -1.0, 1.0);
        let stud_mul = 1.0 + cap * (0.09 + 0.05 * bevel_light) - rim * (0.20 - 0.10 * bevel_light);
        bump_shade *= mix(1.0, stud_mul, stud_fade);
    } else if (local_n.y < -0.85 && bot_surf == 2) {
        let sx = fract(in.local_pos.x + in.part_size.x * 0.5) - 0.5;
        let sz = fract(in.local_pos.z + in.part_size.z * 0.5) - 0.5;
        let r = sqrt(sx * sx + sz * sz);
        let aa = 0.025 + dist * 0.0018;
        let hole = 1.0 - smoothstep(0.23 - aa, 0.25 + aa, r);
        let rim = smoothstep(0.23 - aa, 0.25 + aa, r) * (1.0 - smoothstep(0.30 - aa, 0.32 + aa, r));
        bump_shade *= mix(1.0, 1.0 - hole * 0.18 + rim * 0.08, stud_fade);
    }

    // ---- Procedural Roblox Decals ----
    if (decal_id == 1 && local_n.z < -0.55) {
        // Classic Roblox Face on front (-Z) of Head
        let face_uv = vec2<f32>(
            in.local_pos.x / max(in.part_size.x, 0.01),
            in.local_pos.y / max(in.part_size.y, 0.01),
        );
        let face = eval_classic_face(face_uv);
        albedo = mix(albedo, face.rgb, face.a);
    } else if (decal_id == 2 && local_n.y > 0.85) {
        // Classic Roblox SpawnLocation sunburst decal on top (+Y)
        let sp_uv = vec2<f32>(
            in.local_pos.x / max(in.part_size.x, 0.01),
            in.local_pos.z / max(in.part_size.z, 0.01),
        );
        let sp = eval_spawn_decal(sp_uv);
        albedo = mix(albedo, sp.rgb, sp.a);
    } else if (decal_id == 3) {
        // Classic R6 Torso Shirt (subtle collar cutout & hem)
        let ty = in.local_pos.y / max(in.part_size.y, 0.01); // -0.5..0.5
        let tx = in.local_pos.x / max(in.part_size.x, 0.01);
        // Classic yellow neck cutout on front & top of shirt
        if (local_n.z < -0.5 && ty > 0.36 && abs(tx) < 0.22) {
            let neck_d = length(vec2<f32>(tx / 0.20, (ty - 0.50) / 0.14));
            let neck_m = 1.0 - smoothstep(0.85, 1.05, neck_d);
            let skin = srgb_to_linear(vec3<f32>(0.96, 0.80, 0.19));
            albedo = mix(albedo, skin, neck_m);
        }
        // Subtle dark hem at bottom of shirt
        let hem = smoothstep(-0.50, -0.44, ty);
        bump_shade *= mix(0.82, 1.0, hem);
    } else if (decal_id == 4) {
        // Classic R6 Legs (subtle waistband at top & shoe cuff at bottom)
        let ly = in.local_pos.y / max(in.part_size.y, 0.01);
        let waist = smoothstep(0.42, 0.48, ly);
        let shoe = 1.0 - smoothstep(-0.38, -0.34, ly);
        albedo = mix(albedo, albedo * 0.72, waist * 0.5);
        albedo = mix(albedo, albedo * 0.84, shoe * 0.45);
    }

    // Subtle brick bevel along part edges (only for non-character blocks)
    if (decal_id == 0 && mat_id != 15) {
        let half_sz = max(in.part_size * 0.5, vec3<f32>(0.01));
        let edge_dist = half_sz - abs(in.local_pos);
        let min_edge = min(
            select(edge_dist.x, 99.0, abs(local_n.x) > 0.8),
            min(
                select(edge_dist.y, 99.0, abs(local_n.y) > 0.8),
                select(edge_dist.z, 99.0, abs(local_n.z) > 0.8),
            ),
        );
        let bevel = smoothstep(0.0, 0.045, min_edge);
        bump_shade *= mix(0.86, 1.0, bevel);
    }

    // ---- Lighting & Shadows (Linear Space) ----
    let ndotl = max(dot(base_n, sun_dir), 0.0);
    let shadow = sample_shadow(in.world_pos, base_n, sun_dir);

    let sky_top_lin = srgb_to_linear(frame.sky_top.rgb);
    let sky_bot_lin = srgb_to_linear(frame.sky_bottom.rgb);
    let ambient_lin = srgb_to_linear(frame.ambient.rgb);

    // Hemisphere ambient (blue sky bounce above, warm ground bounce below)
    let hemi = base_n.y * 0.5 + 0.5;
    let hemi_ambient = mix(ambient_lin * 0.72, sky_top_lin * 0.42 + ambient_lin * 0.75, hemi);

    let sun_col = vec3<f32>(1.0, 0.96, 0.88);
    let diffuse = sun_col * (ndotl * shadow * 1.15 * frame.sun_dir.w);

    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(base_n, half_vec), 0.0), spec_power) * shadow * spec_strength * step(0.01, ndotl);

    // Subtle Fresnel sky reflection for glossy / metallic / glass materials
    let fresnel = pow(1.0 - max(dot(base_n, view_dir), 0.0), 4.0);
    let env_refl = mix(sky_bot_lin, sky_top_lin, clamp(base_n.y * 0.5 + 0.5, 0.0, 1.0))
        * (metallic * 0.45 + fresnel * (0.08 + metallic * 0.35));

    var lit = albedo * (hemi_ambient + diffuse) * bump_shade
        + sun_col * spec
        + env_refl
        + emissive;

    // Place-accurate distance fog (crisp on Crossroads, gentle in the far distance)
    let fog_start = frame.fog_params.x;
    let fog_end = max(frame.fog_params.y, fog_start + 80.0);
    let fog_t = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 0.72);
    lit = mix(lit, sky_bot_lin, fog_t);

    // Soft highlight roll-off (Reinhard-like shoulder so bright whites & neon stay clean)
    lit = lit / (vec3<f32>(1.0) + lit * 0.22);

    return vec4<f32>(lit, alpha);
}

// -------------------- 4. CLASSIC ROBLOX COREGUI + MOBILE HUD --------------------

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

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

fn blend_over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let a = src.a + dst.a * (1.0 - src.a);
    let rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / max(a, 1e-5);
    return vec4<f32>(rgb, a);
}

fn get_slot_icon(slot: i32) -> i32 {
    if (slot == 0) { return i32(round(frame.hud_tools_a.x)); }
    if (slot == 1) { return i32(round(frame.hud_tools_a.y)); }
    if (slot == 2) { return i32(round(frame.hud_tools_a.z)); }
    if (slot == 3) { return i32(round(frame.hud_tools_a.w)); }
    if (slot == 4) { return i32(round(frame.hud_tools_b.x)); }
    return i32(round(frame.hud_tools_b.y));
}

// Crisp procedural classic Roblox tool icons keyed by ToolIconKind (from StarterPack)
fn eval_tool_icon(p: vec2<f32>, icon_kind: i32) -> vec4<f32> {
    var col = vec4<f32>(0.0);
    if (icon_kind == 1) {
        // Classic LinkedSword (diagonal silver blade + gold guard + grip)
        let blade = sd_segment(p, vec2<f32>(-0.22, 0.22), vec2<f32>(0.48, -0.48));
        let guard = sd_segment(p, vec2<f32>(-0.36, 0.04), vec2<f32>(-0.04, 0.36));
        let grip = sd_segment(p, vec2<f32>(-0.46, 0.46), vec2<f32>(-0.20, 0.20));
        if (blade < 0.10) {
            let m = 1.0 - smoothstep(0.06, 0.10, blade);
            col = blend_over(col, vec4<f32>(0.78, 0.84, 0.90, m));
        }
        if (grip < 0.07) {
            let m = 1.0 - smoothstep(0.04, 0.07, grip);
            col = blend_over(col, vec4<f32>(0.18, 0.14, 0.10, m));
        }
        if (guard < 0.065) {
            let m = 1.0 - smoothstep(0.04, 0.065, guard);
            col = blend_over(col, vec4<f32>(0.92, 0.72, 0.15, m));
        }
    } else if (icon_kind == 2) {
        // Classic Slingshot (wooden Y-fork + elastic band)
        let stem = sd_segment(p, vec2<f32>(0.0, 0.52), vec2<f32>(0.0, 0.02));
        let fork_l = sd_segment(p, vec2<f32>(0.0, 0.04), vec2<f32>(-0.34, -0.44));
        let fork_r = sd_segment(p, vec2<f32>(0.0, 0.04), vec2<f32>(0.34, -0.44));
        let band = sd_segment(p, vec2<f32>(-0.32, -0.38), vec2<f32>(0.32, -0.38));
        let wood = min(stem, min(fork_l, fork_r));
        if (band < 0.045) {
            col = blend_over(col, vec4<f32>(0.75, 0.20, 0.15, 1.0 - smoothstep(0.025, 0.045, band)));
        }
        if (wood < 0.085) {
            col = blend_over(col, vec4<f32>(0.45, 0.25, 0.10, 1.0 - smoothstep(0.055, 0.085, wood)));
        }
    } else if (icon_kind == 3) {
        // Classic Rocket Launcher (olive tube + warhead tip)
        let tube = sd_segment(p, vec2<f32>(-0.42, 0.24), vec2<f32>(0.38, -0.22));
        let tip = length(p - vec2<f32>(0.44, -0.26));
        if (tube < 0.16) {
            col = blend_over(col, vec4<f32>(0.24, 0.34, 0.18, 1.0 - smoothstep(0.12, 0.16, tube)));
        }
        if (tip < 0.15) {
            col = blend_over(col, vec4<f32>(0.85, 0.18, 0.12, 1.0 - smoothstep(0.11, 0.15, tip)));
        }
    } else if (icon_kind == 4) {
        // Classic Trowel (builder tool)
        let trowel_grip = sd_segment(p, vec2<f32>(-0.44, 0.42), vec2<f32>(-0.08, 0.08));
        let blade = sd_rounded_box(p - vec2<f32>(0.20, -0.18), vec2<f32>(0.26, 0.20), 0.06);
        if (trowel_grip < 0.075) {
            col = blend_over(col, vec4<f32>(0.48, 0.26, 0.10, 1.0 - smoothstep(0.05, 0.075, trowel_grip)));
        }
        if (blade < 0.02) {
            col = blend_over(col, vec4<f32>(0.70, 0.75, 0.80, 1.0 - smoothstep(-0.02, 0.02, blade)));
        }
    } else if (icon_kind == 5) {
        // Classic Timebomb (black sphere + orange fuse spark)
        let body = length(p - vec2<f32>(-0.04, 0.10));
        let fuse = sd_segment(p, vec2<f32>(0.12, -0.22), vec2<f32>(0.36, -0.44));
        let spark = length(p - vec2<f32>(0.38, -0.46));
        if (fuse < 0.05) {
            col = blend_over(col, vec4<f32>(0.55, 0.45, 0.32, 1.0 - smoothstep(0.03, 0.05, fuse)));
        }
        if (body < 0.38) {
            let m = 1.0 - smoothstep(0.34, 0.38, body);
            let hl = 1.0 - smoothstep(0.08, 0.20, length(p - vec2<f32>(-0.16, -0.04)));
            col = blend_over(col, vec4<f32>(vec3<f32>(0.08, 0.09, 0.11) + vec3<f32>(0.25) * hl, m));
        }
        if (spark < 0.14) {
            col = blend_over(col, vec4<f32>(1.0, 0.62, 0.08, 1.0 - smoothstep(0.06, 0.14, spark)));
        }
    } else if (icon_kind == 6) {
        // Classic Superball (glossy red ball)
        let r = length(p);
        if (r < 0.42) {
            let m = 1.0 - smoothstep(0.38, 0.42, r);
            let hl = 1.0 - smoothstep(0.08, 0.22, length(p - vec2<f32>(-0.14, -0.14)));
            col = blend_over(col, vec4<f32>(vec3<f32>(0.85, 0.10, 0.08) + vec3<f32>(0.55) * hl, m));
        }
    } else {
        // Generic Tool / HopperBin (classic Roblox gear/block icon)
        let box_d = sd_rounded_box(p, vec2<f32>(0.32, 0.32), 0.08);
        if (box_d < 0.02) {
            col = blend_over(col, vec4<f32>(0.72, 0.78, 0.86, 1.0 - smoothstep(-0.02, 0.02, box_d)));
        }
    }
    return col;
}

@fragment
fn fs_hud(in: HudOutput) -> @location(0) vec4<f32> {
    let w = max(frame.hud_screen.x, 1.0);
    let h = max(frame.hud_screen.y, 1.0);
    let px = in.uv * vec2<f32>(w, h);
    let short_side = min(w, h);
    let ui_scale = clamp(short_side / 540.0, 0.65, 1.8);

    var out_col = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // ---- A. Classic Top-Right Roblox Health Bar ----
    let hb_w = 176.0 * ui_scale;
    let hb_h = 26.0 * ui_scale;
    let hb_center = vec2<f32>(w - 18.0 * ui_scale - hb_w * 0.5, 16.0 * ui_scale + hb_h * 0.5);
    let hb_d = sd_rounded_box(px - hb_center, vec2<f32>(hb_w * 0.5, hb_h * 0.5), 6.0 * ui_scale);
    if (hb_d < 2.0) {
        let bg_m = 1.0 - smoothstep(-1.0, 1.0, hb_d);
        let border_m = smoothstep(-2.0, -0.5, hb_d) * bg_m;
        out_col = blend_over(out_col, vec4<f32>(0.04, 0.05, 0.07, bg_m * 0.72));
        out_col = blend_over(out_col, vec4<f32>(0.80, 0.85, 0.92, border_m * 0.35));

        // Red cross emblem on left side of health pill
        let cross_c = vec2<f32>(hb_center.x - hb_w * 0.5 + 14.0 * ui_scale, hb_center.y);
        let cp = (px - cross_c) / (6.5 * ui_scale);
        let c_h = sd_rounded_box(cp, vec2<f32>(0.95, 0.34), 0.12);
        let c_v = sd_rounded_box(cp, vec2<f32>(0.34, 0.95), 0.12);
        let c_m = 1.0 - smoothstep(-0.08, 0.12, min(c_h, c_v));
        out_col = blend_over(out_col, vec4<f32>(0.92, 0.22, 0.22, c_m * 0.95));

        // Inner green health fill bar
        let hp = clamp(frame.hud_core.x, 0.0, 1.0);
        let bar_left = hb_center.x - hb_w * 0.5 + 26.0 * ui_scale;
        let bar_w = hb_w - 34.0 * ui_scale;
        let bar_h = 11.0 * ui_scale;
        let track_c = vec2<f32>(bar_left + bar_w * 0.5, hb_center.y);
        let track_d = sd_rounded_box(px - track_c, vec2<f32>(bar_w * 0.5, bar_h * 0.5), 3.0 * ui_scale);
        let track_m = 1.0 - smoothstep(-0.8, 0.8, track_d);
        out_col = blend_over(out_col, vec4<f32>(0.12, 0.14, 0.18, track_m * 0.85));

        if (hp > 0.01) {
            let fill_w = max(bar_w * hp, 4.0 * ui_scale);
            let fill_c = vec2<f32>(bar_left + fill_w * 0.5, hb_center.y);
            let fill_d = sd_rounded_box(px - fill_c, vec2<f32>(fill_w * 0.5, bar_h * 0.5), 3.0 * ui_scale);
            let fill_m = 1.0 - smoothstep(-0.8, 0.8, fill_d);
            let hp_col = mix(vec3<f32>(0.88, 0.16, 0.12), vec3<f32>(0.12, 0.78, 0.28), smoothstep(0.2, 0.75, hp));
            out_col = blend_over(out_col, vec4<f32>(hp_col, fill_m * 0.95));
        }
    }

    // ---- B. Classic Bottom-Center Roblox Tool Hotbar (Backpack) ----
    let tool_count = clamp(i32(round(frame.hud_core.y)), 0, 6);
    let active_tool = i32(round(frame.hud_core.z));
    if (tool_count > 0) {
        let slot_sz = 46.0 * ui_scale;
        let slot_gap = 7.0 * ui_scale;
        let total_w = f32(tool_count) * slot_sz + f32(tool_count - 1) * slot_gap;
        let start_x = (w - total_w) * 0.5 + slot_sz * 0.5;
        let bar_y = h - 16.0 * ui_scale - slot_sz * 0.5;

        if (abs(px.y - bar_y) < slot_sz * 0.65 && abs(px.x - w * 0.5) < total_w * 0.55) {
            for (var i = 0; i < 6; i = i + 1) {
                if (i >= tool_count) {
                    break;
                }
                let sc = vec2<f32>(start_x + f32(i) * (slot_sz + slot_gap), bar_y);
                let d = sd_rounded_box(px - sc, vec2<f32>(slot_sz * 0.5), 7.0 * ui_scale);
                if (d < 2.0) {
                    let is_active = (i == active_tool);
                    let bg_m = 1.0 - smoothstep(-1.0, 1.0, d);
                    let border_w = select(1.6, 2.6, is_active) * ui_scale;
                    let border_m = smoothstep(-border_w - 0.8, -border_w + 0.5, d) * bg_m;
                    let bg_col = select(vec3<f32>(0.04, 0.05, 0.08), vec3<f32>(0.10, 0.16, 0.24), is_active);
                    let bd_col = select(vec3<f32>(0.65, 0.72, 0.82), vec3<f32>(0.25, 0.75, 1.0), is_active);
                    out_col = blend_over(out_col, vec4<f32>(bg_col, bg_m * select(0.68, 0.82, is_active)));
                    out_col = blend_over(out_col, vec4<f32>(bd_col, border_m * select(0.45, 0.95, is_active)));

                    // Procedural classic tool icon mapped from StarterPack ToolIconKind
                    let icon_uv = (px - sc) / (slot_sz * 0.42);
                    let icon = eval_tool_icon(icon_uv, get_slot_icon(i));
                    out_col = blend_over(out_col, icon);

                    // Top-left slot indicator dot
                    let dot_p = px - (sc + vec2<f32>(-slot_sz * 0.34, -slot_sz * 0.34));
                    let dot_m = 1.0 - smoothstep(2.2 * ui_scale, 3.2 * ui_scale, length(dot_p));
                    out_col = blend_over(out_col, vec4<f32>(1.0, 1.0, 1.0, dot_m * select(0.55, 0.95, is_active)));
                }
            }
        }
    }

    // ---- C. Mobile Virtual Thumbstick & Jump Button (only on touch devices) ----
    if (frame.hud_core.w > 0.5) {
        let stick_r = short_side * 0.115;
        let stick_center = vec2<f32>(
            frame.hud_stick.x * w,
            frame.hud_stick.y * h,
        );
        let knob_center = stick_center + frame.hud_stick.zw * stick_r;
        let d_stick = length(px - stick_center);
        let d_knob = length(px - knob_center);

        if (d_stick < stick_r + 3.0) {
            let fill_a = select(0.18, 0.28, frame.hud_screen.z > 0.5);
            let ring = smoothstep(stick_r - 3.0, stick_r - 1.0, d_stick) * (1.0 - smoothstep(stick_r, stick_r + 2.5, d_stick));
            let fill = 1.0 - smoothstep(stick_r - 1.5, stick_r + 0.5, d_stick);
            out_col = blend_over(out_col, vec4<f32>(0.05, 0.07, 0.10, fill * fill_a));
            out_col = blend_over(out_col, vec4<f32>(1.0, 1.0, 1.0, ring * 0.55));
        }

        let knob_r = stick_r * 0.42;
        if (d_knob < knob_r + 2.0) {
            let k = 1.0 - smoothstep(knob_r - 1.5, knob_r + 1.0, d_knob);
            let ka = select(0.45, 0.78, frame.hud_screen.z > 0.5) * k;
            out_col = blend_over(out_col, vec4<f32>(1.0, 1.0, 1.0, ka));
        }

        let jump_r = short_side * 0.088;
        let jump_center = vec2<f32>(w - short_side * 0.15, h - short_side * 0.16);
        let d_jump = length(px - jump_center);
        if (d_jump < jump_r + 3.0) {
            let pressed = frame.hud_screen.w > 0.5;
            let fill = 1.0 - smoothstep(jump_r - 1.5, jump_r + 0.5, d_jump);
            let ring = smoothstep(jump_r - 3.0, jump_r - 1.0, d_jump) * (1.0 - smoothstep(jump_r, jump_r + 2.5, d_jump));
            let bg_a = select(0.24, 0.48, pressed) * fill;
            out_col = blend_over(out_col, vec4<f32>(0.04, 0.06, 0.10, bg_a));
            out_col = blend_over(out_col, vec4<f32>(1.0, 1.0, 1.0, ring * 0.65));

            let lp = (px - jump_center) / jump_r;
            let shaft = step(abs(lp.x), 0.11) * step(-0.12, lp.y) * step(lp.y, 0.42);
            let head = step(lp.y, 0.02) * step(-0.46, lp.y) * step(abs(lp.x), (lp.y + 0.46) * 0.85);
            let arrow = clamp(shaft + head, 0.0, 1.0);
            if (arrow > 0.5) {
                out_col = blend_over(out_col, vec4<f32>(1.0, 1.0, 1.0, 0.88));
            }
        }
    }

    if (out_col.a < 0.01) {
        discard;
    }
    return out_col;
}
