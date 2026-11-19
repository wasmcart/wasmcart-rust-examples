//! WGSL. Every fragment shader writes linear colour and encodes sRGB itself
//! when SRGB_ENCODE says the canvas view is not sRGB (compatibility mode).

pub const COMMON: &str = r#"
override SRGB_ENCODE: bool = false;

struct Scene {
    view_proj: mat4x4f,
    cam: vec4f,       // camera position, w = time in seconds
    sun_dir: vec4f,   // toward the light, w = ambient
    sun_col: vec4f,   // w = grid scroll
    fog: vec4f,       // fog colour, w = fog distance
    sky_top: vec4f,
    accent: vec4f,    // canyon line colour, w = strength
    cam_right: vec4f,
    cam_up: vec4f,
    lights: array<vec4f, 16>, // 8 x (position, radius) (colour, intensity)
};
@group(0) @binding(0) var<uniform> scene: Scene;

fn encode(c: vec3f) -> vec3f {
    if (!SRGB_ENCODE) { return c; }
    let x = clamp(c, vec3f(0.0), vec3f(1.0));
    return select(1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055, 12.92 * x, x <= vec3f(0.0031308));
}

fn fogged(c: vec3f, wpos: vec3f) -> vec3f {
    let d = length(wpos - scene.cam.xyz) / scene.fog.w;
    let f = 1.0 - exp(-d * d);
    return mix(c, scene.fog.rgb, f);
}
"#;

pub const LIT: &str = r#"
struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) wpos: vec3f,
    @location(1) n: vec3f,
    @location(2) color: vec4f,
    @location(3) params: vec4f,
    @location(4) lpos: vec3f,
};

@vertex
fn vs(@location(0) p: vec3f, @location(1) n: vec3f,
      @location(2) m0: vec4f, @location(3) m1: vec4f, @location(4) m2: vec4f, @location(5) m3: vec4f,
      @location(6) color: vec4f, @location(7) params: vec4f) -> VOut {
    let m = mat4x4f(m0, m1, m2, m3);
    let w = m * vec4f(p, 1.0);
    // M = R*S: R*(n/s) is the true normal, and M*(n/s^2) equals it.
    let s2 = vec3f(dot(m0.xyz, m0.xyz), dot(m1.xyz, m1.xyz), dot(m2.xyz, m2.xyz));
    let wn = mat3x3f(m0.xyz, m1.xyz, m2.xyz) * (n / max(s2, vec3f(1e-6)));
    var o: VOut;
    o.pos = scene.view_proj * w;
    o.wpos = w.xyz;
    o.n = wn;
    o.color = color;
    o.params = params;
    o.lpos = p;
    return o;
}

@fragment
fn fs(v: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    var n = normalize(v.n);
    if (!front) { n = -n; }
    let kind = v.params.y;
    let t = scene.cam.w;
    let view = normalize(scene.cam.xyz - v.wpos);
    var base = v.color.rgb;
    var emis = vec3f(0.0);
    var shin = 24.0;
    var spec_k = 0.35;
    // Derivatives first: they must run in uniform control flow.
    let gz = (v.wpos.z - scene.sun_col.w) / 8.0;
    let gx = v.wpos.x / 3.0;
    let gy = v.wpos.y / 3.0;
    let fz = fwidth(gz) + 0.002;
    let fx = fwidth(gx);
    let fy = fwidth(gy);

    if (kind < 0.5) {
        // Crystal: pulsing inner glow and a bright rim.
        let pulse = 0.65 + 0.35 * sin(t * 1.7 + v.params.z);
        let rim = pow(1.0 - max(dot(n, view), 0.0), 3.0);
        emis = v.color.rgb * (v.params.x * pulse + rim * min(v.params.x * 1.2, 1.0));
        // Glow rises toward the point.
        emis *= 0.55 + 0.6 * clamp(v.lpos.y, 0.0, 1.0);
        base = v.color.rgb * 0.55;
        shin = 60.0;
        spec_k = 0.9;
    } else if (kind < 1.5) {
        // Canyon rock with glowing survey lines that scroll past.
        let lz = abs(fract(gz) - 0.5);
        let lx = abs(fract(gx) - 0.5);
        let ly = abs(fract(gy) - 0.5);
        // Lines across the flight path stay faint: bright ones would strobe
        // past at speed. Lines along the path never flicker.
        let line = 0.15 * smoothstep(0.47 - fz * 1.5, 0.5, lz)
                 + 0.6 * smoothstep(0.485 - fx * 1.5, 0.5, lx) * step(0.7, n.y)
                 + 0.6 * smoothstep(0.485 - fy * 1.5, 0.5, ly) * (1.0 - step(0.7, n.y));
        let strata = 0.85 + 0.15 * sin(v.wpos.y * 2.3 + sin(gz * 0.88) * 2.0);
        base = v.color.rgb * strata;
        // Lines fade out close to the camera so they never sweep past as bright bands.
        let ld = smoothstep(6.0, 26.0, length(scene.cam.xyz - v.wpos));
        emis = scene.accent.rgb * scene.accent.w * min(line, 1.0) * ld;
        shin = 12.0;
        spec_k = 0.15;
    } else if (kind < 2.5) {
        // Gate ring: mostly light, breathing slowly.
        let pulse = 0.8 + 0.2 * sin(t * 3.0 + v.params.z);
        let rim = pow(1.0 - max(dot(n, view), 0.0), 2.0);
        emis = v.color.rgb * v.params.x * pulse * (0.7 + 0.6 * rim);
        spec_k = 1.0;
        shin = 40.0;
    } else {
        // Glider: polished hull, glowing engine edge at the tail.
        let engine = smoothstep(0.45, 0.62, v.lpos.z);
        emis = v.color.rgb * v.params.x * engine + vec3f(0.3, 0.8, 1.0) * v.params.w;
        shin = 70.0;
        spec_k = 1.2;
    }

    // Sun plus hemisphere ambient.
    let sun = normalize(scene.sun_dir.xyz);
    let amb = scene.sun_dir.w * mix(vec3f(0.35, 0.3, 0.4), scene.sky_top.rgb * 2.0 + vec3f(0.25), n.y * 0.5 + 0.5);
    var light = amb + scene.sun_col.rgb * max(dot(n, sun), 0.0);
    var spec = scene.sun_col.rgb * pow(max(dot(n, normalize(sun + view)), 0.0), shin) * 0.5;
    for (var i = 0u; i < 8u; i++) {
        let lp = scene.lights[i * 2u];
        let lc = scene.lights[i * 2u + 1u];
        if (lc.w <= 0.0) { continue; }
        let d = lp.xyz - v.wpos;
        let dist = length(d);
        let l = d / max(dist, 1e-4);
        let fall = clamp(1.0 - dist / lp.w, 0.0, 1.0);
        let att = lc.w * fall * fall;
        light += lc.rgb * att * (max(dot(n, l), 0.0) * 0.85 + 0.15);
        spec += lc.rgb * att * pow(max(dot(n, normalize(l + view)), 0.0), shin);
    }
    // Glow eases off right next to the camera, so crystals sweeping past
    // never pop bright across a large part of the screen.
    let near = 0.25 + 0.75 * smoothstep(4.0, 18.0, length(scene.cam.xyz - v.wpos));
    var c = base * light + (spec * spec_k + emis) * near;
    // Crystals as a whole dim as they sweep past the lens.
    if (kind < 0.5) { c *= 0.2 + 0.8 * smoothstep(4.0, 26.0, length(scene.cam.xyz - v.wpos)); }
    return vec4f(encode(fogged(c, v.wpos)), 1.0);
}
"#;

pub const SKY: &str = r#"
struct SOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> SOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: SOut;
    o.pos = vec4f(p, 0.0, 1.0);
    o.uv = p;
    return o;
}

@fragment
fn fs(v: SOut) -> @location(0) vec4f {
    // Horizon sits a little above screen centre (the camera looks slightly down).
    let h = clamp((v.uv.y - 0.12) * 1.4, 0.0, 1.0);
    var c = mix(scene.fog.rgb, scene.sky_top.rgb, pow(h, 0.7));
    // A soft glow band on the horizon, the well's light far ahead.
    let band = exp(-pow((v.uv.y - 0.12) * 5.0, 2.0)) * exp(-v.uv.x * v.uv.x * 1.5);
    c += scene.accent.rgb * band * 0.35;
    return vec4f(encode(c), 1.0);
}
"#;

pub const PARTICLE: &str = r#"
struct POut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec3f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32,
      @location(0) pl: vec4f, @location(1) pv: vec4f, @location(2) col: vec4f) -> POut {
    var corners = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
                                  vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));
    let c = corners[vi];
    var o: POut;
    let life = pl.w;
    var size = col.w * select(0.0, 1.0, life > 0.0);
    // Stretch along the motion so fast sparks streak.
    let w = pl.xyz + scene.cam_right.xyz * c.x * size + scene.cam_up.xyz * c.y * size
          + pv.xyz * 0.025 * (c.y * 0.5 + 0.5);
    o.pos = scene.view_proj * vec4f(w, 1.0);
    o.uv = c;
    let d = length(pl.xyz - scene.cam.xyz) / scene.fog.w;
    // Sparks fade before they reach the lens: no big bright blobs.
    let cd = length(pl.xyz - scene.cam.xyz);
    let fade = exp(-d * d * 1.3) * clamp(life, 0.0, 1.0) * smoothstep(2.0, 7.0, cd);
    o.color = col.rgb * fade;
    return o;
}

@fragment
fn fs(v: POut) -> @location(0) vec4f {
    let r2 = dot(v.uv, v.uv);
    let a = exp(-r2 * 3.5) * (1.0 - smoothstep(0.8, 1.0, r2));
    return vec4f(encode(v.color * a), 1.0);
}
"#;

pub const HUD: &str = r#"
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct HOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32,
      @location(0) rect: vec4f, @location(1) uvr: vec4f, @location(2) color: vec4f) -> HOut {
    var corners = array<vec2f, 6>(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
                                  vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
    let c = corners[vi];
    let px = rect.xy + c * rect.zw;
    var o: HOut;
    o.pos = vec4f(px.x / 480.0 - 1.0, 1.0 - px.y / 270.0, 0.0, 1.0);
    o.uv = (uvr.xy + c * uvr.zw) / vec2f(128.0, 64.0);
    o.color = color;
    return o;
}

@fragment
fn fs(v: HOut) -> @location(0) vec4f {
    let a = textureSample(atlas, samp, v.uv).a * v.color.a;
    return vec4f(encode(v.color.rgb), a);
}
"#;

pub const SIM: &str = r#"
struct P { pos: vec3f, life: f32, vel: vec3f, seq: f32, col: vec4f };
struct Sim {
    a: vec4f,          // dt, time, forward shift, frame
    glider: vec4f,     // position, w = trail on
    glider_vel: vec4f,
    mote_col: vec4f,
    trail_col: vec4f,
    bursts: array<vec4f, 16>, // 8 x (position, seq) (colour, kind)
};
@group(0) @binding(0) var<uniform> sim: Sim;
@group(0) @binding(1) var<storage, read_write> ps: array<P>;

const MOTES: u32 = 3072u;
const TRAIL: u32 = 1024u;
const PER_BURST: u32 = 256u;
const TOTAL: u32 = 6144u;

fn hash(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}
fn rnd(i: u32, k: u32) -> f32 {
    return f32(hash(i * 9781u + k * 6271u + 17u) & 0xffffffu) / 16777216.0;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let i = id.x;
    if (i >= TOTAL) { return; }
    var p = ps[i];
    let dt = sim.a.x;
    let dz = sim.a.z;
    let tk = u32(sim.a.w);

    if (i < MOTES) {
        if (p.seq == 0.0) {
            p.pos = vec3f(rnd(i, 1u) * 56.0 - 28.0, rnd(i, 2u) * 22.0 - 7.0, -rnd(i, 3u) * 250.0 + 8.0);
            p.vel = vec3f(rnd(i, 4u) - 0.5, rnd(i, 5u) * 0.4 + 0.1, rnd(i, 6u) - 0.5) * 0.6;
            p.seq = 1.0;
            p.life = 1.0;
            p.col = vec4f(1.0, 1.0, 1.0, 0.05 + 0.1 * rnd(i, 7u));
        }
        p.pos += p.vel * dt;
        p.pos.z += dz;
        if (p.pos.z > 8.0 || p.pos.y > 16.0) {
            p.pos = vec3f(rnd(i, tk) * 56.0 - 28.0, rnd(i, tk + 1u) * 22.0 - 7.0, p.pos.z - 250.0);
            if (p.pos.z < -245.0) { p.pos.z = -240.0; }
        }
        // Twinkle slowly; brightness from the sector's mote colour.
        let tw = 0.55 + 0.45 * sin(sim.a.y * 1.3 + f32(i) * 1.7);
        p.col = vec4f(sim.mote_col.rgb * (0.4 + 0.6 * rnd(i, 8u)) * tw, p.col.w);
    } else if (i < MOTES + TRAIL) {
        if (p.seq == 0.0) {
            p.seq = 1.0 + rnd(i, 1u);
            p.life = -1.0;
        }
        p.life -= dt * (1.4 + p.seq * 0.8);
        if (p.life <= 0.0 && sim.glider.w > 0.5) {
            let j = vec3f(rnd(i, tk) - 0.5, rnd(i, tk + 3u) - 0.5, rnd(i, tk + 5u));
            // Two engine nozzles at the wing roots.
            let side = select(-0.32, 0.32, rnd(i, tk + 7u) > 0.5);
            p.pos = sim.glider.xyz + vec3f(side + j.x * 0.12, 0.02 + j.y * 0.1, 0.62 + j.z * 0.3);
            p.vel = vec3f(j.x * 1.2, j.y * 1.2, 2.0 + j.z * 3.0) - sim.glider_vel.xyz * 0.3;
            p.life = 0.6 + 0.4 * rnd(i, tk + 11u);
            p.col = vec4f(sim.trail_col.rgb * 0.7, 0.03 + 0.03 * rnd(i, tk + 13u));
        }
        p.pos += p.vel * dt;
        p.pos.z += dz;
    } else {
        let k = (i - MOTES - TRAIL) / PER_BURST;
        let b = sim.bursts[k * 2u];
        let bc = sim.bursts[k * 2u + 1u];
        if (b.w != p.seq && b.w > 0.0) {
            p.seq = b.w;
            let a = rnd(i, tk) * 6.2831853;
            let e = rnd(i, tk + 1u) * 2.0 - 1.0;
            var dir: vec3f;
            var speed: f32;
            if (bc.w < 0.5) {
                // Gate: a ring flung outward in the gate's plane.
                dir = vec3f(cos(a), sin(a), e * 0.25);
                speed = 5.0 + rnd(i, tk + 2u) * 7.0;
                p.pos = b.xyz + vec3f(cos(a), sin(a), 0.0) * 1.5;
            } else {
                // Crash: a sphere of shards.
                let r = sqrt(1.0 - e * e);
                dir = vec3f(cos(a) * r, e, sin(a) * r);
                speed = 3.0 + rnd(i, tk + 2u) * 9.0;
                p.pos = b.xyz + dir * 0.3;
            }
            p.vel = dir * speed;
            p.life = 0.7 + 0.5 * rnd(i, tk + 3u);
            p.col = vec4f(bc.rgb * (0.6 + 0.6 * rnd(i, tk + 4u)), 0.06 + 0.1 * rnd(i, tk + 5u));
        }
        p.life -= dt * 0.9;
        p.vel *= (1.0 - 1.8 * dt);
        p.pos += p.vel * dt;
        p.pos.z += dz;
    }
    ps[i] = p;
}
"#;
