//! GPU version of the video effect chain (`wgpu` compute shaders).
//!
//! The picture is uploaded once as premultiplied floats, the chain runs as a series of compute
//! passes that ping-pong between three buffers, and the result is read back once. The maths is the
//! one of the CPU reference in [`crate::vfx`] (same kernels, same vignette formula), and a test
//! compares the two. Pictures that do not fit the device's buffer limits return an error so the
//! caller falls back to the CPU.

use std::borrow::Cow;
use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::frame::FrameBuffer;
use crate::transitions as tr;
use crate::vfx::{
    contrast_gain, gaussian_kernel, hue_degrees, linear_blur_line, median_radius, offset_pixels,
    pixelate_block, range_pixels, vignette_corner, Resolved, Work, UNSHARP_RADIUS,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    half: u32,
    _pad: u32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
    i: f32,
    j: f32,
    k: f32,
    l: f32,
}

impl Params {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            half: 0,
            _pad: 0,
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            h: 0.0,
            i: 0.0,
            j: 0.0,
            k: 0.0,
            l: 0.0,
        }
    }
}

const WGSL: &str = r#"
struct Params {
    width: u32,
    height: u32,
    half: u32,
    pad: u32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
    i: f32,
    j: f32,
    k: f32,
    l: f32,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> weights: array<f32>;
@group(0) @binding(4) var<storage, read> aux: array<vec4<f32>>;

fn at(x: i32, y: i32) -> vec4<f32> {
    let cx = clamp(x, 0, i32(p.width) - 1);
    let cy = clamp(y, 0, i32(p.height) - 1);
    return src[u32(cy) * p.width + u32(cx)];
}

fn clamp_pm(v: vec4<f32>) -> vec4<f32> {
    let a = clamp(v.w, 0.0, 1.0);
    return vec4<f32>(clamp(v.x, 0.0, a), clamp(v.y, 0.0, a), clamp(v.z, 0.0, a), a);
}

@compute @workgroup_size(8, 8, 1)
fn blur_h(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    var acc = vec4<f32>(0.0);
    let n = i32(2u * p.half + 1u);
    for (var k = 0; k < n; k = k + 1) {
        acc = acc + weights[k] * at(i32(id.x) + k - i32(p.half), i32(id.y));
    }
    dst[id.y * p.width + id.x] = acc;
}

@compute @workgroup_size(8, 8, 1)
fn blur_v(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    var acc = vec4<f32>(0.0);
    let n = i32(2u * p.half + 1u);
    for (var k = 0; k < n; k = k + 1) {
        acc = acc + weights[k] * at(i32(id.x), i32(id.y) + k - i32(p.half));
    }
    dst[id.y * p.width + id.x] = acc;
}

@compute @workgroup_size(8, 8, 1)
fn sharpen(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let x = i32(id.x);
    let y = i32(id.y);
    let v = 5.0 * at(x, y) - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1);
    dst[id.y * p.width + id.x] = clamp_pm(v);
}

// src = blurred picture, aux = original picture, a = ratio
@compute @workgroup_size(8, 8, 1)
fn unsharp_mix(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let o = aux[i];
    let b = src[i];
    let v = o + p.a * (o - b);
    dst[i] = clamp_pm(vec4<f32>(v.x, v.y, v.z, o.w));
}

// a = outer, b = inner, c = aspect, d/e = centre, f = corner distance, g = frame aspect ratio
@compute @workgroup_size(8, 8, 1)
fn vignette(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let nx = (f32(id.x) + 0.5) / f32(p.width) - p.d;
    let ny = 1.0 - (f32(id.y) + 0.5) / f32(p.height) - p.e;
    let r = sqrt(pow((nx * p.g) / p.c, 2.0) + ny * ny) / p.f;
    let t = clamp((r - p.b) / (p.a - p.b), 0.0, 1.0);
    let gain = 1.0 - t * t * (3.0 - 2.0 * t);
    let s = src[i];
    dst[i] = vec4<f32>(s.x * gain, s.y * gain, s.z * gain, s.w);
}

// a = amount
@compute @workgroup_size(8, 8, 1)
fn black_and_white(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    let l = s.x * 0.2126 + s.y * 0.7152 + s.z * 0.0722;
    dst[i] = vec4<f32>(s.x + p.a * (l - s.x), s.y + p.a * (l - s.y), s.z + p.a * (l - s.z), s.w);
}

// a = amount
@compute @workgroup_size(8, 8, 1)
fn invert(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    let n = vec3<f32>(s.w - s.x, s.w - s.y, s.w - s.z);
    let c = s.xyz + p.a * (n - s.xyz);
    dst[i] = vec4<f32>(c.x, c.y, c.z, s.w);
}

// a = brightness, b = contrast gain
@compute @workgroup_size(8, 8, 1)
fn brightness_contrast(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) {
        dst[i] = s;
        return;
    }
    let v = clamp((s.xyz / s.w - vec3<f32>(0.5)) * p.b + vec3<f32>(0.5 + p.a), vec3<f32>(0.0), vec3<f32>(1.0));
    dst[i] = vec4<f32>(v.x * s.w, v.y * s.w, v.z * s.w, s.w);
}

// a = block width, b = block height (pixels)
@compute @workgroup_size(8, 8, 1)
fn pixelate(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let bw = u32(p.a);
    let bh = u32(p.b);
    let sx = min(id.x / bw * bw + bw / 2u, p.width - 1u);
    let sy = min(id.y / bh * bh + bh / 2u, p.height - 1u);
    dst[id.y * p.width + id.x] = src[sy * p.width + sx];
}

fn sample_bilinear(x: f32, y: f32) -> vec4<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    var acc = vec4<f32>(0.0);
    for (var j = 0; j < 2; j = j + 1) {
        for (var i = 0; i < 2; i = i + 1) {
            let px = i32(x0) + i;
            let py = i32(y0) + j;
            if (px < 0 || py < 0 || px >= i32(p.width) || py >= i32(p.height)) { continue; }
            var wx = 1.0 - tx;
            if (i == 1) { wx = tx; }
            var wy = 1.0 - ty;
            if (j == 1) { wy = ty; }
            acc = acc + wx * wy * src[u32(py) * p.width + u32(px)];
        }
    }
    return acc;
}

// a = angle, b/c = centre (fractions of the frame, lower-left origin)
@compute @workgroup_size(8, 8, 1)
fn mirror(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let r = radians(p.a);
    let n = vec2<f32>(-sin(r), cos(r));
    let q = vec2<f32>(f32(id.x) + 0.5, h - (f32(id.y) + 0.5));
    let side = dot(q - vec2<f32>(p.b * w, p.c * h), n);
    let i = id.y * p.width + id.x;
    if (side >= 0.0) {
        dst[i] = src[i];
    } else {
        let rr = q - 2.0 * side * n;
        dst[i] = sample_bilinear(rr.x, h - rr.y);
    }
}

// a = strength, b/c/d = filter colour
@compute @workgroup_size(8, 8, 1)
fn sepia(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) {
        dst[i] = s;
        return;
    }
    let straight = s.xyz / s.w;
    let l = straight.x * 0.2126 + straight.y * 0.7152 + straight.z * 0.0722;
    let tint = vec3<f32>(p.b, p.c, p.d);
    var toned = tint * 2.0 * l;
    if (l >= 0.5) {
        toned = tint + (vec3<f32>(1.0) - tint) * (2.0 * l - 1.0);
    }
    let c = (straight + p.a * (toned - straight)) * s.w;
    dst[i] = vec4<f32>(c.x, c.y, c.z, s.w);
}

// a/b = centre in pixels (y down), c = ratio, d/e = radii in pixels
@compute @workgroup_size(8, 8, 1)
fn swirl(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let dx = f32(id.x) + 0.5 - p.a;
    let dy = f32(id.y) + 0.5 - p.b;
    let r = sqrt(pow(dx / p.d, 2.0) + pow(dy / p.e, 2.0));
    if (r >= 1.0) {
        dst[i] = src[i];
        return;
    }
    let turn = p.c * 6.283185307179586 * (1.0 - r) * (1.0 - r);
    let sn = sin(turn);
    let cs = cos(turn);
    dst[i] = sample_bilinear(p.a + cs * dx - sn * dy, p.b + sn * dx + cs * dy);
}

// a/b = centre in pixels (y down), c = level; 24 taps, last one at 0.4 of the way at level 1
@compute @workgroup_size(8, 8, 1)
fn radial_blur(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let qx = f32(id.x) + 0.5;
    let qy = f32(id.y) + 0.5;
    var acc = vec4<f32>(0.0);
    for (var k = 0; k < 24; k = k + 1) {
        let s = 1.0 - p.c * 0.4 * f32(k) / 23.0;
        acc = acc + sample_bilinear(p.a + (qx - p.a) * s, p.b + (qy - p.b) * s);
    }
    dst[id.y * p.width + id.x] = acc / 24.0;
}

// a/b = half width and height of the window in pixels (at most 4)
@compute @workgroup_size(8, 8, 1)
fn median(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let rx = i32(p.a);
    let ry = i32(p.b);
    let n = (2 * rx + 1) * (2 * ry + 1);
    var out = vec4<f32>(0.0);
    for (var ch = 0; ch < 4; ch = ch + 1) {
        var arr: array<f32, 81>;
        var m = 0;
        for (var dy = -ry; dy <= ry; dy = dy + 1) {
            for (var dx = -rx; dx <= rx; dx = dx + 1) {
                let v = at(i32(id.x) + dx, i32(id.y) + dy);
                var key = v.x;
                if (ch == 1) { key = v.y; }
                if (ch == 2) { key = v.z; }
                if (ch == 3) { key = v.w; }
                var j = m - 1;
                loop {
                    if (j < 0 || arr[j] <= key) { break; }
                    arr[j + 1] = arr[j];
                    j = j - 1;
                }
                arr[j + 1] = key;
                m = m + 1;
            }
        }
        let value = arr[n / 2];
        if (ch == 0) { out.x = value; }
        if (ch == 1) { out.y = value; }
        if (ch == 2) { out.z = value; }
        if (ch == 3) { out.w = value; }
    }
    dst[id.y * p.width + id.x] = out;
}

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(max(c.x, c.y), c.z);
    let mn = min(min(c.x, c.y), c.z);
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if (d < 0.000001) { return vec3<f32>(0.0, 0.0, l); }
    let s = d / max(1.0 - abs(2.0 * l - 1.0), 0.000001);
    var h = 0.0;
    if (mx == c.x) {
        let q = (c.y - c.z) / d;
        h = q - 6.0 * floor(q / 6.0);
    } else if (mx == c.y) {
        h = (c.z - c.x) / d + 2.0;
    } else {
        h = (c.x - c.y) / d + 4.0;
    }
    return vec3<f32>(h * 60.0, s, l);
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = hsl.x - 360.0 * floor(hsl.x / 360.0);
    let c = (1.0 - abs(2.0 * hsl.z - 1.0)) * hsl.y;
    let hp = h / 60.0;
    let x = c * (1.0 - abs((hp - 2.0 * floor(hp / 2.0)) - 1.0));
    var rgb = vec3<f32>(c, 0.0, x);
    let sector = u32(hp);
    if (sector == 0u) { rgb = vec3<f32>(c, x, 0.0); }
    else if (sector == 1u) { rgb = vec3<f32>(x, c, 0.0); }
    else if (sector == 2u) { rgb = vec3<f32>(0.0, c, x); }
    else if (sector == 3u) { rgb = vec3<f32>(0.0, x, c); }
    else if (sector == 4u) { rgb = vec3<f32>(x, 0.0, c); }
    let m = hsl.z - c / 2.0;
    return rgb + vec3<f32>(m);
}

// a = hue offset in degrees, b = saturation, c = luminosity
@compute @workgroup_size(8, 8, 1)
fn hsl_adjust(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) {
        dst[i] = s;
        return;
    }
    var hsl = rgb_to_hsl(s.xyz / s.w);
    hsl.x = hsl.x + p.a;
    hsl.y = clamp(hsl.y * p.b, 0.0, 1.0);
    hsl.z = clamp(hsl.z * p.c, 0.0, 1.0);
    let rgb = clamp(hsl_to_rgb(hsl), vec3<f32>(0.0), vec3<f32>(1.0)) * s.w;
    dst[i] = vec4<f32>(rgb.x, rgb.y, rgb.z, s.w);
}

// a/b = unit vector of the line (y down), c = half length in pixels; 33 taps
@compute @workgroup_size(8, 8, 1)
fn linear_blur(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let qx = f32(id.x) + 0.5;
    let qy = f32(id.y) + 0.5;
    let max_x = f32(p.width) - 0.5;
    let max_y = f32(p.height) - 0.5;
    var acc = vec4<f32>(0.0);
    for (var k = 0; k <= 32; k = k + 1) {
        let t = (f32(k) - 16.0) / 16.0 * p.c;
        acc = acc + sample_bilinear(clamp(qx + p.a * t, 0.5, max_x), clamp(qy + p.b * t, 0.5, max_y));
    }
    dst[id.y * p.width + id.x] = acc / 33.0;
}

fn straight_color(s: vec4<f32>, c: vec3<f32>) -> vec4<f32> {
    let o = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * s.w;
    return vec4<f32>(o.x, o.y, o.z, s.w);
}

fn luma_straight(c: vec3<f32>) -> f32 {
    return c.x * 0.2126 + c.y * 0.7152 + c.z * 0.0722;
}

// a/b/c = shadows, d/e/f = midtones, g/h/i = highlights, j = preserve the luminosity
@compute @workgroup_size(8, 8, 1)
fn color_balance(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let l = luma_straight(c);
    let ws = (1.0 - l) * (1.0 - l);
    let wh = l * l;
    let wm = max(1.0 - ws - wh, 0.0);
    var o = c + vec3<f32>(p.a, p.b, p.c) * ws + vec3<f32>(p.d, p.e, p.f) * wm + vec3<f32>(p.g, p.h, p.i) * wh;
    if (p.j >= 0.5) {
        o = o + vec3<f32>(l - luma_straight(o));
    }
    dst[i] = straight_color(s, o);
}

// a/b = input black and white, c/d = output black and white, e = gamma exponent
@compute @workgroup_size(8, 8, 1)
fn levels(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let t = clamp((s.xyz / s.w - vec3<f32>(p.a)) / (p.b - p.a), vec3<f32>(0.0), vec3<f32>(1.0));
    dst[i] = straight_color(s, vec3<f32>(p.c) + pow(t, vec3<f32>(p.e)) * (p.d - p.c));
}

// a = exponent
@compute @workgroup_size(8, 8, 1)
fn gamma_correction(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    dst[i] = straight_color(s, pow(max(s.xyz / s.w, vec3<f32>(0.0)), vec3<f32>(p.a)));
}

// a = number of steps
@compute @workgroup_size(8, 8, 1)
fn posterize(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let v = s.xyz / s.w * p.a;
    dst[i] = straight_color(s, vec3<f32>(floor(v.x + 0.5), floor(v.y + 0.5), floor(v.z + 0.5)) / p.a);
}

// a = low cut, b = high cut, c = softness, d = invert
@compute @workgroup_size(8, 8, 1)
fn extract(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let l = luma_straight(s.xyz / s.w);
    var inside = 0.0;
    if (p.c <= 0.0) {
        if (l > p.a && l < p.b) { inside = 1.0; }
    } else {
        inside = min(clamp((l - p.a) / p.c, 0.0, 1.0), clamp((p.b - l) / p.c, 0.0, 1.0));
    }
    if (p.d >= 0.5) { inside = 1.0 - inside; }
    dst[i] = straight_color(s, vec3<f32>(inside));
}

// a/b/c = colour for black, d/e/f = colour for white, g = amount
@compute @workgroup_size(8, 8, 1)
fn tint(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let l = luma_straight(c);
    let black = vec3<f32>(p.a, p.b, p.c);
    let white = vec3<f32>(p.d, p.e, p.f);
    dst[i] = straight_color(s, c + (black + (white - black) * l - c) * p.g);
}

fn hue_frac(c: vec3<f32>) -> f32 {
    return rgb_to_hsl(c).x / 360.0;
}

fn hue_dist(a: f32, b: f32) -> f32 {
    let d = abs(a - b);
    return min(d, 1.0 - d);
}

// a..d = red row, e..h = green row, i..l = blue row (red, green, blue, constant)
@compute @workgroup_size(8, 8, 1)
fn channel_mix(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let o = vec3<f32>(
        p.a * c.x + p.b * c.y + p.c * c.z + p.d,
        p.e * c.x + p.f * c.y + p.g * c.z + p.h,
        p.i * c.x + p.j * c.y + p.k * c.z + p.l
    );
    dst[i] = straight_color(s, o);
}

// a/b/c = slope, d/e/f = offset, g/h/i = power, j = saturation
@compute @workgroup_size(8, 8, 1)
fn asc_cdl(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let t = clamp(c * vec3<f32>(p.a, p.b, p.c) + vec3<f32>(p.d, p.e, p.f), vec3<f32>(0.0), vec3<f32>(1.0));
    let v = pow(t, max(vec3<f32>(p.g, p.h, p.i), vec3<f32>(0.0)));
    let l = luma_straight(v);
    dst[i] = straight_color(s, vec3<f32>(l) + (v - vec3<f32>(l)) * p.j);
}

// a = amount, b = key hue, c = tolerance, d = softness
@compute @workgroup_size(8, 8, 1)
fn leave_color(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let d = hue_dist(hue_frac(c), p.b) * 2.0;
    let keep = 1.0 - clamp((d - p.c) / p.d, 0.0, 1.0);
    let l = luma_straight(c);
    let k = p.a * (1.0 - keep);
    dst[i] = straight_color(s, c + (vec3<f32>(l) - c) * k);
}

// a = from hue, b = to hue, c = tolerance, d = softness
@compute @workgroup_size(8, 8, 1)
fn change_to_color(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    var hsl = rgb_to_hsl(s.xyz / s.w);
    let h = hsl.x / 360.0;
    let w = 1.0 - clamp((hue_dist(h, p.a) - p.c) / p.d, 0.0, 1.0);
    let moved = h + (p.b - p.a) * w;
    hsl.x = (moved - floor(moved)) * 360.0;
    dst[i] = straight_color(s, hsl_to_rgb(hsl));
}

// a/b/c = key colour, d = similarity (already by 1.0), e = reverse
@compute @workgroup_size(8, 8, 1)
fn color_pass(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    if (s.w <= 0.000001) { dst[i] = s; return; }
    let c = s.xyz / s.w;
    let d = length(c - vec3<f32>(p.a, p.b, p.c));
    let near = d <= p.d * 1.2;
    let keep = near != (p.e >= 0.5);
    if (keep) {
        dst[i] = straight_color(s, c);
    } else {
        dst[i] = straight_color(s, vec3<f32>(luma_straight(c)));
    }
}

// a = opacity, b = ignore the alpha, c = invert it, d = the matte alone
@compute @workgroup_size(8, 8, 1)
fn alpha_adjust(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let s = src[i];
    var c = vec3<f32>(0.0);
    if (s.w > 0.000001) { c = s.xyz / s.w; }
    var a = s.w;
    if (p.b >= 0.5) { a = 1.0; }
    if (p.c >= 0.5) { a = 1.0 - a; }
    a = clamp(a * p.a, 0.0, 1.0);
    if (p.d >= 0.5) {
        dst[i] = vec4<f32>(a, a, a, 1.0);
    } else {
        dst[i] = vec4<f32>(c.x * a, c.y * a, c.z * a, a);
    }
}

@compute @workgroup_size(8, 8, 1)
fn hflip(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    dst[id.y * p.width + id.x] = src[id.y * p.width + (p.width - 1u - id.x)];
}

@compute @workgroup_size(8, 8, 1)
fn vflip(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    dst[id.y * p.width + id.x] = src[(p.height - 1u - id.y) * p.width + id.x];
}

// a/b = shift in pixels (y down), c = mix with the original
@compute @workgroup_size(8, 8, 1)
fn offset_fx(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let u = f32(id.x) + 0.5 - p.a;
    let v = f32(id.y) + 0.5 - p.b;
    let uw = clamp(u - w * floor(u / w), 0.5, w - 0.5);
    let vw = clamp(v - h * floor(v / h), 0.5, h - 0.5);
    let shifted = sample_bilinear(uw, vw);
    let i = id.y * p.width + id.x;
    dst[i] = shifted + (src[i] - shifted) * p.c;
}

fn gradient_value(pattern: u32, u: f32, v: f32, aspect: f32) -> f32 {
    if (pattern == 0u) { return u; }
    if (pattern == 1u) { return v; }
    if (pattern == 2u) { return (u * aspect + v) / (aspect + 1.0); }
    let rad = length(vec2<f32>((u - 0.5) * aspect, v - 0.5)) / length(vec2<f32>(aspect / 2.0, 0.5));
    if (pattern == 3u) { return rad; }
    if (pattern == 4u) { return 1.0 - rad; }
    if (pattern == 5u) {
        let turn = atan2((u - 0.5) * aspect, -(v - 0.5)) / 6.283185307179586;
        return turn - floor(turn);
    }
    return (abs(u - 0.5) * aspect + abs(v - 0.5)) / (aspect / 2.0 + 0.5);
}

// The clip before (binding 4), sampled like `sample_bilinear` samples the clip it reveals
fn sample_aux(x: f32, y: f32) -> vec4<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    var acc = vec4<f32>(0.0);
    for (var j = 0; j < 2; j = j + 1) {
        for (var i = 0; i < 2; i = i + 1) {
            let px = i32(x0) + i;
            let py = i32(y0) + j;
            if (px < 0 || py < 0 || px >= i32(p.width) || py >= i32(p.height)) { continue; }
            var wx = 1.0 - tx;
            if (i == 1) { wx = tx; }
            var wy = 1.0 - ty;
            if (j == 1) { wy = ty; }
            acc = acc + wx * wy * aux[u32(py) * p.width + u32(px)];
        }
    }
    return acc;
}

fn over(top: vec4<f32>, bottom: vec4<f32>) -> vec4<f32> {
    return top + bottom * (1.0 - top.w);
}

// Distance from the side a transition enters from and the position across it
fn edge_travel(edge: u32, x: f32, y: f32, w: f32, h: f32) -> vec2<f32> {
    if (edge == 0u) { return vec2<f32>(x, y); }
    if (edge == 1u) { return vec2<f32>(y, x); }
    if (edge == 2u) { return vec2<f32>(w - x, y); }
    return vec2<f32>(h - y, x);
}

fn edge_point(edge: u32, s: f32, lat: f32, w: f32, h: f32) -> vec2<f32> {
    if (edge == 0u) { return vec2<f32>(s, lat); }
    if (edge == 1u) { return vec2<f32>(lat, s); }
    if (edge == 2u) { return vec2<f32>(w - s, lat); }
    return vec2<f32>(lat, h - s);
}

fn edge_dir(edge: u32) -> vec2<f32> {
    if (edge == 1u) { return vec2<f32>(0.0, 1.0); }
    if (edge == 2u) { return vec2<f32>(-1.0, 0.0); }
    if (edge == 3u) { return vec2<f32>(0.0, -1.0); }
    return vec2<f32>(1.0, 0.0);
}

// Transitions. a = progress, b = frame aspect ratio, c = angle (start angle for the clock wipe),
// d = softness, e = blind count, half = mode (0 dissolve, 1 linear wipe, 2 clock wipe, 3 iris,
// 4 barn door, 5 blinds)
fn reveal(s: f32, progress: f32, soft: f32) -> f32 {
    let sf = max(soft, 0.002);
    let t = clamp((progress * (1.0 + sf) - s) / sf, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn along(u: f32, v: f32, aspect: f32, degrees: f32) -> f32 {
    let r = radians(degrees);
    let sn = sin(r);
    let cs = cos(r);
    let c0 = 0.0;
    let c1 = aspect * cs;
    let c2 = sn;
    let c3 = aspect * cs + sn;
    let lo = min(min(c0, c1), min(c2, c3));
    let hi = max(max(c0, c1), max(c2, c3));
    if (hi - lo < 0.000001) { return 0.0; }
    return clamp((u * aspect * cs + v * sn - lo) / (hi - lo), 0.0, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn trans_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let u = (f32(id.x) + 0.5) / f32(p.width);
    let v = (f32(id.y) + 0.5) / f32(p.height);
    let aspect = p.b;
    var m = p.a;
    if (p.half == 1u) {
        m = reveal(along(u, v, aspect, p.c), p.a, p.d);
    } else if (p.half == 2u) {
        let dx = (u - 0.5) * aspect;
        let dy = v - 0.5;
        let turn = atan2(dx, -dy) / 6.283185307179586;
        let q = turn - p.c / 360.0;
        m = reveal(q - floor(q), p.a, p.d);
    } else if (p.half == 3u) {
        let r = length(vec2<f32>((u - 0.5) * aspect, v - 0.5)) / length(vec2<f32>(aspect / 2.0, 0.5));
        m = reveal(r, p.a, p.d);
    } else if (p.half == 4u) {
        m = reveal(abs(along(u, v, aspect, p.c) - 0.5) * 2.0, p.a, p.d);
    } else if (p.half == 5u) {
        m = reveal(fract(along(u, v, aspect, p.c) * p.e), p.a, p.d);
    } else if (p.half == 6u) {
        let cx = p.c;
        let cy = p.e;
        let ex = max(cx, 1.0 - cx) * aspect;
        let ey = max(cy, 1.0 - cy);
        m = reveal(max(abs(u - cx) * aspect / ex, abs(v - cy) / ey), p.a, p.d);
    } else if (p.half == 7u) {
        var g = gradient_value(u32(p.c + 0.5), u, v, aspect);
        if (p.e >= 0.5) { g = 1.0 - g; }
        m = reveal(g, p.a, p.d);
    }
    let i = id.y * p.width + id.x;
    dst[i] = src[i] * m;
}

// c = cosine and d = sine of the angle (computed on the CPU: a GPU cosine of 0 is not exactly 1,
// and at the edge of the frame that leaks a sliver of the picture)
@compute @workgroup_size(8, 8, 1)
fn trans_slide(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let ox = -p.c * (1.0 - p.a) * f32(p.width);
    let oy = -p.d * (1.0 - p.a) * f32(p.height);
    dst[id.y * p.width + id.x] = sample_bilinear(f32(id.x) + 0.5 - ox, f32(id.y) + 0.5 - oy);
}

// a = progress, b/c = centre in pixels (y down), d/e = distance from the centre to the far edge
@compute @workgroup_size(8, 8, 1)
fn trans_split(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    var sx = -1.0;
    if (px >= p.b) { sx = 1.0; }
    var sy = -1.0;
    if (py >= p.c) { sy = 1.0; }
    let qx = px - sx * (1.0 - p.a) * p.d;
    let qy = py - sy * (1.0 - p.a) * p.e;
    let i = id.y * p.width + id.x;
    if ((qx >= p.b) != (sx > 0.0) || (qy >= p.c) != (sy > 0.0)) {
        dst[i] = vec4<f32>(0.0);
    } else {
        dst[i] = sample_bilinear(qx, qy);
    }
}

@compute @workgroup_size(8, 8, 1)
fn trans_grow(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.a <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let cx = f32(p.width) / 2.0;
    let cy = f32(p.height) / 2.0;
    dst[i] = sample_bilinear(cx + (f32(id.x) + 0.5 - cx) / p.a, cy + (f32(id.y) + 0.5 - cy) / p.a);
}

// a = progress, half = edge (0 left, 1 top, 2 right, 3 bottom)
@compute @workgroup_size(8, 8, 1)
fn trans_squeeze(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.a <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let w = f32(p.width);
    let h = f32(p.height);
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    if (p.half == 0u) {
        dst[i] = sample_bilinear(px / p.a, py);
    } else if (p.half == 1u) {
        dst[i] = sample_bilinear(px, py / p.a);
    } else if (p.half == 2u) {
        dst[i] = sample_bilinear(w - (w - px) / p.a, py);
    } else {
        dst[i] = sample_bilinear(px, h - (h - py) / p.a);
    }
}

// c = cosine, d = sine of the turn left, e = scale (computed on the CPU)
@compute @workgroup_size(8, 8, 1)
fn trans_spiral(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    if (p.e <= 0.001) {
        dst[i] = vec4<f32>(0.0);
        return;
    }
    let cx = f32(p.width) / 2.0;
    let cy = f32(p.height) / 2.0;
    let dx = f32(id.x) + 0.5 - cx;
    let dy = f32(id.y) + 0.5 - cy;
    dst[i] = sample_bilinear(cx + (p.c * dx + p.d * dy) / p.e, cy + (p.c * dy - p.d * dx) / p.e);
}

// a = progress, b = flash strength, c/d/e = flash colour
@compute @workgroup_size(8, 8, 1)
fn trans_flash(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let i = id.y * p.width + id.x;
    let flash = vec4<f32>(p.c * p.b, p.d * p.b, p.e * p.b, p.b);
    dst[i] = flash + src[i] * (p.a * (1.0 - p.b));
}

// Push. a = progress, half = edge, b = border thickness in pixels, c = border feather, d/e/f =
// border colour, g = push the clip before off (1) or let the new one slide over it (0)
@compute @workgroup_size(8, 8, 1)
fn trans_push(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    let edge = p.half;
    var e = h;
    if (edge % 2u == 0u) { e = w; }
    let d = edge_dir(edge);
    let lead = p.a * e;
    let entering = (1.0 - p.a) * e;
    let inc = sample_bilinear(px + d.x * entering, py + d.y * entering);
    var prev = sample_aux(px, py);
    if (p.g >= 0.5) { prev = sample_aux(px - d.x * lead, py - d.y * lead); }
    var below = prev;
    let s = edge_travel(edge, px, py, w, h).x;
    if (p.b > 0.0 && p.a > 0.0 && s >= lead && s < lead + p.b) {
        let ramp = p.c * p.b;
        var a = 1.0;
        if (ramp > 0.0) { a = clamp((lead + p.b - s) / ramp, 0.0, 1.0); }
        below = over(vec4<f32>(p.d * a, p.e * a, p.f * a, a), prev);
    }
    dst[id.y * p.width + id.x] = over(inc, below);
}

// Page roll. half = edge, b = where the front of the roll is, c = the radius it has by now
@compute @workgroup_size(8, 8, 1)
fn trans_roll(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    let st = edge_travel(p.half, px, py, w, h);
    let inc = sample_bilinear(px, py);
    var plain = vec4<f32>(0.0);
    if (st.x >= p.b) { plain = sample_aux(px, py); }
    var shown = plain;
    if (p.c > 0.0 && abs(st.x - p.b) <= p.c) {
        let u = clamp((p.b - st.x) / p.c, -1.0, 1.0);
        let arc = p.c * (3.141592653589793 - asin(u));
        let source = p.b - arc;
        if (source >= 0.0) {
            let at = edge_point(p.half, source, st.y, w, h);
            let o = sample_aux(at.x, at.y);
            let k = 0.35 + 0.65 * sqrt(1.0 - u * u);
            shown = over(vec4<f32>(o.x * k, o.y * k, o.z * k, o.w), plain);
        }
    }
    if (p.h >= 0.5) {
        dst[id.y * p.width + id.x] = inc * (1.0 - shown.w);
    } else {
        dst[id.y * p.width + id.x] = over(shown, inc);
    }
}

// Page peel. half = edge, b = where the fold is
@compute @workgroup_size(8, 8, 1)
fn trans_peel(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.width || id.y >= p.height) { return; }
    let w = f32(p.width);
    let h = f32(p.height);
    let px = f32(id.x) + 0.5;
    let py = f32(id.y) + 0.5;
    let st = edge_travel(p.half, px, py, w, h);
    let inc = sample_bilinear(px, py);
    let i = id.y * p.width + id.x;
    if (st.x < p.b) {
        dst[i] = inc;
        return;
    }
    let plain = sample_aux(px, py);
    var shown = plain;
    if (st.x <= 2.0 * p.b) {
        let at = edge_point(p.half, 2.0 * p.b - st.x, st.y, w, h);
        let o = sample_aux(at.x, at.y);
        shown = over(vec4<f32>(o.x * 0.6, o.y * 0.6, o.z * 0.6, o.w), plain);
    }
    if (p.h >= 0.5) {
        dst[i] = inc * (1.0 - shown.w);
    } else {
        dst[i] = over(shown, inc);
    }
}
"#;

/// The compute pipelines of the effect chain, built once per device.
pub struct VfxGpu {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    blur_h: wgpu::ComputePipeline,
    blur_v: wgpu::ComputePipeline,
    sharpen: wgpu::ComputePipeline,
    unsharp_mix: wgpu::ComputePipeline,
    vignette: wgpu::ComputePipeline,
    black_and_white: wgpu::ComputePipeline,
    invert: wgpu::ComputePipeline,
    brightness_contrast: wgpu::ComputePipeline,
    pixelate: wgpu::ComputePipeline,
    mirror: wgpu::ComputePipeline,
    sepia: wgpu::ComputePipeline,
    swirl: wgpu::ComputePipeline,
    radial_blur: wgpu::ComputePipeline,
    median: wgpu::ComputePipeline,
    hsl_adjust: wgpu::ComputePipeline,
    linear_blur: wgpu::ComputePipeline,
    color_balance: wgpu::ComputePipeline,
    levels: wgpu::ComputePipeline,
    gamma_correction: wgpu::ComputePipeline,
    posterize: wgpu::ComputePipeline,
    extract: wgpu::ComputePipeline,
    tint: wgpu::ComputePipeline,
    channel_mix: wgpu::ComputePipeline,
    asc_cdl: wgpu::ComputePipeline,
    leave_color: wgpu::ComputePipeline,
    change_to_color: wgpu::ComputePipeline,
    color_pass: wgpu::ComputePipeline,
    alpha_adjust: wgpu::ComputePipeline,
    hflip: wgpu::ComputePipeline,
    vflip: wgpu::ComputePipeline,
    offset_fx: wgpu::ComputePipeline,
    trans_mask: wgpu::ComputePipeline,
    trans_slide: wgpu::ComputePipeline,
    trans_grow: wgpu::ComputePipeline,
    trans_squeeze: wgpu::ComputePipeline,
    trans_spiral: wgpu::ComputePipeline,
    trans_flash: wgpu::ComputePipeline,
    trans_split: wgpu::ComputePipeline,
    trans_push: wgpu::ComputePipeline,
    trans_roll: wgpu::ComputePipeline,
    trans_peel: wgpu::ComputePipeline,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Which pipeline a pass runs.
#[derive(Clone, Copy)]
enum Pass {
    BlurH,
    BlurV,
    Sharpen,
    UnsharpMix,
    Vignette,
    BlackAndWhite,
    Invert,
    BrightnessContrast,
    Pixelate,
    Mirror,
    Sepia,
    Swirl,
    RadialBlur,
    Median,
    HslAdjust,
    LinearBlur,
    ColorBalance,
    Levels,
    Gamma,
    Posterize,
    Extract,
    Tint,
    ChannelMix,
    AscCdl,
    LeaveColor,
    ChangeToColor,
    ColorKeep,
    AlphaAdjust,
    HFlip,
    VFlip,
    OffsetFx,
    TransMask,
    TransSlide,
    TransGrow,
    TransSqueeze,
    TransSpiral,
    TransFlash,
    TransSplit,
    TransPush,
    TransRoll,
    TransPeel,
}

/// One step of a GPU run: an effect, or a transition at a progress.
#[derive(Clone, Copy)]
enum Op {
    Effect(Resolved),
    Transition(tr::Resolved, f32),
}

impl VfxGpu {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("neoveg_vfx_wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(WGSL)),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("neoveg_vfx_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(3, true),
                storage_entry(4, true),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("neoveg_vfx_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let make = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &shader,
                entry_point: entry,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        Self {
            blur_h: make("blur_h"),
            blur_v: make("blur_v"),
            sharpen: make("sharpen"),
            unsharp_mix: make("unsharp_mix"),
            vignette: make("vignette"),
            black_and_white: make("black_and_white"),
            invert: make("invert"),
            brightness_contrast: make("brightness_contrast"),
            pixelate: make("pixelate"),
            mirror: make("mirror"),
            sepia: make("sepia"),
            swirl: make("swirl"),
            radial_blur: make("radial_blur"),
            median: make("median"),
            hsl_adjust: make("hsl_adjust"),
            linear_blur: make("linear_blur"),
            color_balance: make("color_balance"),
            levels: make("levels"),
            gamma_correction: make("gamma_correction"),
            posterize: make("posterize"),
            extract: make("extract"),
            tint: make("tint"),
            channel_mix: make("channel_mix"),
            asc_cdl: make("asc_cdl"),
            leave_color: make("leave_color"),
            change_to_color: make("change_to_color"),
            color_pass: make("color_pass"),
            alpha_adjust: make("alpha_adjust"),
            hflip: make("hflip"),
            vflip: make("vflip"),
            offset_fx: make("offset_fx"),
            trans_mask: make("trans_mask"),
            trans_slide: make("trans_slide"),
            trans_grow: make("trans_grow"),
            trans_squeeze: make("trans_squeeze"),
            trans_spiral: make("trans_spiral"),
            trans_flash: make("trans_flash"),
            trans_split: make("trans_split"),
            trans_push: make("trans_push"),
            trans_roll: make("trans_roll"),
            trans_peel: make("trans_peel"),
            device,
            queue,
            bgl,
        }
    }

    fn pipeline(&self, pass: Pass) -> &wgpu::ComputePipeline {
        match pass {
            Pass::BlurH => &self.blur_h,
            Pass::BlurV => &self.blur_v,
            Pass::Sharpen => &self.sharpen,
            Pass::UnsharpMix => &self.unsharp_mix,
            Pass::Vignette => &self.vignette,
            Pass::BlackAndWhite => &self.black_and_white,
            Pass::Invert => &self.invert,
            Pass::BrightnessContrast => &self.brightness_contrast,
            Pass::Pixelate => &self.pixelate,
            Pass::Mirror => &self.mirror,
            Pass::Sepia => &self.sepia,
            Pass::Swirl => &self.swirl,
            Pass::RadialBlur => &self.radial_blur,
            Pass::Median => &self.median,
            Pass::HslAdjust => &self.hsl_adjust,
            Pass::LinearBlur => &self.linear_blur,
            Pass::ColorBalance => &self.color_balance,
            Pass::Levels => &self.levels,
            Pass::Gamma => &self.gamma_correction,
            Pass::Posterize => &self.posterize,
            Pass::Extract => &self.extract,
            Pass::Tint => &self.tint,
            Pass::ChannelMix => &self.channel_mix,
            Pass::AscCdl => &self.asc_cdl,
            Pass::LeaveColor => &self.leave_color,
            Pass::ChangeToColor => &self.change_to_color,
            Pass::ColorKeep => &self.color_pass,
            Pass::AlphaAdjust => &self.alpha_adjust,
            Pass::HFlip => &self.hflip,
            Pass::VFlip => &self.vflip,
            Pass::OffsetFx => &self.offset_fx,
            Pass::TransMask => &self.trans_mask,
            Pass::TransSlide => &self.trans_slide,
            Pass::TransGrow => &self.trans_grow,
            Pass::TransSqueeze => &self.trans_squeeze,
            Pass::TransSpiral => &self.trans_spiral,
            Pass::TransFlash => &self.trans_flash,
            Pass::TransSplit => &self.trans_split,
            Pass::TransPush => &self.trans_push,
            Pass::TransRoll => &self.trans_roll,
            Pass::TransPeel => &self.trans_peel,
        }
    }

    /// Runs the resolved chain on `frame` and returns the result. `Err` when the picture does not
    /// fit the device's limits or the read-back fails; the caller then uses the CPU.
    pub fn process(&self, frame: &FrameBuffer, chain: &[Resolved]) -> Result<FrameBuffer, String> {
        let ops: Vec<Op> = chain.iter().map(|e| Op::Effect(*e)).collect();
        self.process_ops(frame, &ops, None)
    }

    /// The layer as it looks at `progress` of the transition `t` (the same picture as
    /// [`tr::apply`]). `Err` as for [`VfxGpu::process`].
    pub fn process_transition(
        &self,
        frame: &FrameBuffer,
        t: &tr::Resolved,
        progress: f32,
    ) -> Result<FrameBuffer, String> {
        let progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if progress >= 1.0 {
            return Ok(frame.clone());
        }
        self.process_ops(frame, &[Op::Transition(*t, progress)], None)
    }

    /// Like [`VfxGpu::process_transition`] for a transition that sees the clip before the one it
    /// reveals: `outgoing` (`None` when there is none) and `incoming` have the size of the canvas.
    pub fn process_transition2(
        &self,
        outgoing: Option<&FrameBuffer>,
        incoming: &FrameBuffer,
        t: &tr::Resolved,
        progress: f32,
    ) -> Result<FrameBuffer, String> {
        let progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if progress >= 1.0 {
            return Ok(incoming.clone());
        }
        self.process_ops(incoming, &[Op::Transition(*t, progress)], outgoing)
    }

    fn process_ops(
        &self,
        frame: &FrameBuffer,
        ops: &[Op],
        second: Option<&FrameBuffer>,
    ) -> Result<FrameBuffer, String> {
        if ops.is_empty() {
            return Ok(frame.clone());
        }
        let (w, h) = (frame.width, frame.height);
        if w == 0 || h == 0 {
            return Ok(frame.clone());
        }
        let bytes = u64::from(w) * u64::from(h) * 16;
        let limits = self.device.limits();
        if bytes > u64::from(limits.max_storage_buffer_binding_size)
            || bytes > limits.max_buffer_size
        {
            return Err("quadro grande demais para os efeitos na GPU".into());
        }

        let work = Work::from_frame(frame);
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let first = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vfx_a"),
                contents: bytemuck::cast_slice(&work.px),
                usage,
            });
        let make_empty = |label| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes,
                usage,
                mapped_at_creation: false,
            })
        };
        let mut bufs = vec![first, make_empty("vfx_b"), make_empty("vfx_c")];
        // The clip before, for the transitions that draw it (nothing when there is none)
        let two_layer = ops.iter().any(|op| {
            matches!(
                op,
                Op::Transition(
                    tr::Resolved::Push { .. }
                        | tr::Resolved::PageRoll { .. }
                        | tr::Resolved::PagePeel { .. },
                    _
                )
            )
        });
        let sheet = ops.iter().any(|op| {
            matches!(
                op,
                Op::Transition(
                    tr::Resolved::PageRoll { .. } | tr::Resolved::PagePeel { .. },
                    _
                )
            )
        });
        if two_layer {
            bufs.push(match second.filter(|s| s.width == w && s.height == h) {
                Some(s) => self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vfx_outgoing"),
                        contents: bytemuck::cast_slice(&Work::from_frame(s).px),
                        usage,
                    }),
                // With no clip before, a roll or a peel uses an opaque sheet (see `apply2`)
                None if sheet => {
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("vfx_sheet"),
                            contents: bytemuck::cast_slice(&vec![
                                [0.0f32, 0.0, 0.0, 1.0];
                                (w * h) as usize
                            ]),
                            usage,
                        })
                }
                None => make_empty("vfx_outgoing"),
            });
        }
        // Never bound for reading and writing in the same pass, so a tiny buffer stands in for an
        // input a pass does not use.
        let dummy = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vfx_dummy"),
                contents: &[0u8; 16],
                usage: wgpu::BufferUsages::STORAGE,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vfx_encoder"),
            });
        let mut cur = 0usize;
        let run = |encoder: &mut wgpu::CommandEncoder,
                   pass: Pass,
                   params: Params,
                   src: usize,
                   dst: usize,
                   weights: Option<&[f32]>,
                   aux: Option<usize>| {
            let params_buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vfx_params"),
                    contents: bytemuck::bytes_of(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let weights_buf = weights.map(|w| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vfx_weights"),
                        contents: bytemuck::cast_slice(w),
                        usage: wgpu::BufferUsages::STORAGE,
                    })
            });
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx_bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bufs[src].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bufs[dst].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: weights_buf
                            .as_ref()
                            .map_or_else(|| dummy.as_entire_binding(), |b| b.as_entire_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: aux.map_or_else(
                            || dummy.as_entire_binding(),
                            |i| bufs[i].as_entire_binding(),
                        ),
                    },
                ],
            });
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("vfx_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(self.pipeline(pass));
            cpass.set_bind_group(0, &bg, &[]);
            cpass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        };

        for op in ops {
            let effect = match *op {
                Op::Effect(e) => e,
                Op::Transition(t, progress) => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = progress;
                    p.b = w as f32 / h as f32;
                    let pass = match t {
                        tr::Resolved::Slide { angle } => {
                            (p.d, p.c) = angle.to_radians().sin_cos();
                            Pass::TransSlide
                        }
                        tr::Resolved::Grow => Pass::TransGrow,
                        tr::Resolved::Split {
                            center,
                            push,
                            softness,
                        } => {
                            if push {
                                let (cx, cy) = (center[0] * w as f32, (1.0 - center[1]) * h as f32);
                                (p.b, p.c) = (cx, cy);
                                (p.d, p.e) = (cx.max(w as f32 - cx), cy.max(h as f32 - cy));
                                Pass::TransSplit
                            } else {
                                (p.half, p.c, p.d, p.e) = (6, center[0], softness, 1.0 - center[1]);
                                Pass::TransMask
                            }
                        }
                        tr::Resolved::GradientWipe {
                            pattern,
                            softness,
                            invert,
                        } => {
                            (p.half, p.c, p.d, p.e) =
                                (7, pattern as f32, softness, f32::from(invert));
                            Pass::TransMask
                        }
                        tr::Resolved::Push {
                            angle,
                            push_off,
                            border,
                            feather,
                            color,
                        } => {
                            p.half = tr::squeeze_edge(angle);
                            p.b = border * h as f32;
                            p.c = feather;
                            (p.d, p.e, p.f) = (color[0], color[1], color[2]);
                            p.g = f32::from(push_off);
                            Pass::TransPush
                        }
                        tr::Resolved::PageRoll { angle, radius } => {
                            let edge = tr::squeeze_edge(angle);
                            let e = (if edge.is_multiple_of(2) { w } else { h }) as f32;
                            (p.b, p.c) = tr::roll_front(e, radius * e, progress);
                            p.half = edge;
                            p.h = f32::from(second.is_none());
                            Pass::TransRoll
                        }
                        tr::Resolved::PagePeel { angle } => {
                            let edge = tr::squeeze_edge(angle);
                            let e = (if edge.is_multiple_of(2) { w } else { h }) as f32;
                            p.b = progress * e;
                            p.half = edge;
                            p.h = f32::from(second.is_none());
                            Pass::TransPeel
                        }
                        tr::Resolved::Squeeze { angle } => {
                            p.half = tr::squeeze_edge(angle);
                            Pass::TransSqueeze
                        }
                        tr::Resolved::Spiral {
                            turns,
                            zoom,
                            clockwise,
                        } => {
                            (p.c, p.d, p.e) =
                                tr::spiral_transform(turns, zoom, clockwise, progress);
                            Pass::TransSpiral
                        }
                        tr::Resolved::Flash { color } => {
                            p.b = tr::flash_strength(progress);
                            (p.c, p.d, p.e) = (color[0], color[1], color[2]);
                            Pass::TransFlash
                        }
                        tr::Resolved::Dissolve => Pass::TransMask,
                        tr::Resolved::LinearWipe { angle, softness } => {
                            (p.half, p.c, p.d) = (1, angle, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::ClockWipe { start, softness } => {
                            (p.half, p.c, p.d) = (2, start, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::Iris { softness } => {
                            (p.half, p.d) = (3, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::BarnDoor { angle, softness } => {
                            (p.half, p.c, p.d) = (4, angle, softness);
                            Pass::TransMask
                        }
                        tr::Resolved::Blinds {
                            count,
                            angle,
                            softness,
                        } => {
                            (p.half, p.c, p.d, p.e) = (5, angle, softness, count);
                            Pass::TransMask
                        }
                    };
                    let aux = matches!(pass, Pass::TransPush | Pass::TransRoll | Pass::TransPeel)
                        .then_some(3);
                    run(&mut encoder, pass, p, cur, dst, None, aux);
                    cur = dst;
                    continue;
                }
            };
            match effect {
                Resolved::GaussianBlur { h_range, v_range } => {
                    for (range, pass) in [(h_range, Pass::BlurH), (v_range, Pass::BlurV)] {
                        if let Some(k) = gaussian_kernel(range_pixels(range, h)) {
                            let dst = (cur + 1) % 3;
                            let mut p = Params::new(w, h);
                            p.half = (k.len() / 2) as u32;
                            run(&mut encoder, pass, p, cur, dst, Some(&k), None);
                            cur = dst;
                        }
                    }
                }
                Resolved::Sharpen => {
                    let dst = (cur + 1) % 3;
                    run(
                        &mut encoder,
                        Pass::Sharpen,
                        Params::new(w, h),
                        cur,
                        dst,
                        None,
                        None,
                    );
                    cur = dst;
                }
                Resolved::UnsharpMask { ratio } => {
                    let Some(k) = gaussian_kernel(range_pixels(UNSHARP_RADIUS * 2.0, h)) else {
                        continue;
                    };
                    let (tmp, blurred) = ((cur + 1) % 3, (cur + 2) % 3);
                    let mut p = Params::new(w, h);
                    p.half = (k.len() / 2) as u32;
                    run(&mut encoder, Pass::BlurH, p, cur, tmp, Some(&k), None);
                    run(&mut encoder, Pass::BlurV, p, tmp, blurred, Some(&k), None);
                    let mut mix = Params::new(w, h);
                    mix.a = ratio;
                    // The free buffer `tmp` takes the result; `cur` is still the original (aux)
                    run(
                        &mut encoder,
                        Pass::UnsharpMix,
                        mix,
                        blurred,
                        tmp,
                        None,
                        Some(cur),
                    );
                    cur = tmp;
                }
                Resolved::Vignette {
                    outer,
                    inner,
                    aspect,
                    center,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = outer;
                    p.b = inner;
                    p.c = aspect;
                    p.d = center[0];
                    p.e = center[1];
                    p.f = vignette_corner(w, h);
                    p.g = w as f32 / h as f32;
                    run(&mut encoder, Pass::Vignette, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::BlackAndWhite { amount } | Resolved::Invert { amount } => {
                    let pass = if matches!(effect, Resolved::Invert { .. }) {
                        Pass::Invert
                    } else {
                        Pass::BlackAndWhite
                    };
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = amount;
                    run(&mut encoder, pass, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::BrightnessContrast {
                    brightness,
                    contrast,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = brightness;
                    p.b = contrast_gain(contrast);
                    run(
                        &mut encoder,
                        Pass::BrightnessContrast,
                        p,
                        cur,
                        dst,
                        None,
                        None,
                    );
                    cur = dst;
                }
                Resolved::Pixelate { h_size, v_size } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = pixelate_block(h_size, h) as f32;
                    p.b = pixelate_block(v_size, h) as f32;
                    run(&mut encoder, Pass::Pixelate, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Mirror { angle, center } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = angle;
                    p.b = center[0];
                    p.c = center[1];
                    run(&mut encoder, Pass::Mirror, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Sepia { tint, strength } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = strength;
                    (p.b, p.c, p.d) = (tint[0], tint[1], tint[2]);
                    run(&mut encoder, Pass::Sepia, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Swirl {
                    center,
                    ratio,
                    size,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = center[0] * w as f32;
                    p.b = (1.0 - center[1]) * h as f32;
                    p.c = ratio;
                    p.d = size[0] * h as f32;
                    p.e = size[1] * h as f32;
                    run(&mut encoder, Pass::Swirl, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::RadialBlur { center, level } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = center[0] * w as f32;
                    p.b = (1.0 - center[1]) * h as f32;
                    p.c = level;
                    run(&mut encoder, Pass::RadialBlur, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Median { h_range, v_range } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = median_radius(h_range, h) as f32;
                    p.b = median_radius(v_range, h) as f32;
                    run(&mut encoder, Pass::Median, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::HslAdjust {
                    add_to_hue,
                    saturation,
                    luminosity,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = hue_degrees(add_to_hue);
                    p.b = saturation;
                    p.c = luminosity;
                    run(&mut encoder, Pass::HslAdjust, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::LinearBlur { angle, ratio } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c) = linear_blur_line(angle, ratio, h);
                    run(&mut encoder, Pass::LinearBlur, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::ColorBalance {
                    shadows,
                    mids,
                    highlights,
                    preserve,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c) = (shadows[0], shadows[1], shadows[2]);
                    (p.d, p.e, p.f) = (mids[0], mids[1], mids[2]);
                    (p.g, p.h, p.i) = (highlights[0], highlights[1], highlights[2]);
                    p.j = f32::from(preserve);
                    run(&mut encoder, Pass::ColorBalance, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Levels { ib, iw, ob, ow, g } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d, p.e) = (ib, iw, ob, ow, g);
                    run(&mut encoder, Pass::Levels, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Gamma { g } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = g;
                    run(&mut encoder, Pass::Gamma, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Posterize { n } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    p.a = n;
                    run(&mut encoder, Pass::Posterize, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Extract {
                    lo,
                    hi,
                    soft,
                    invert,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (lo, hi, soft, f32::from(invert));
                    run(&mut encoder, Pass::Extract, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Tint {
                    black,
                    white,
                    amount,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c) = (black[0], black[1], black[2]);
                    (p.d, p.e, p.f) = (white[0], white[1], white[2]);
                    p.g = amount;
                    run(&mut encoder, Pass::Tint, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::ChannelMix { m } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (m[0][0], m[0][1], m[0][2], m[0][3]);
                    (p.e, p.f, p.g, p.h) = (m[1][0], m[1][1], m[1][2], m[1][3]);
                    (p.i, p.j, p.k, p.l) = (m[2][0], m[2][1], m[2][2], m[2][3]);
                    run(&mut encoder, Pass::ChannelMix, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::AscCdl {
                    slope,
                    offset,
                    power,
                    sat,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c) = (slope[0], slope[1], slope[2]);
                    (p.d, p.e, p.f) = (offset[0], offset[1], offset[2]);
                    (p.g, p.h, p.i) = (power[0], power[1], power[2]);
                    p.j = sat;
                    run(&mut encoder, Pass::AscCdl, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::LeaveColor {
                    amount,
                    key_hue,
                    tol,
                    soft,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (amount, key_hue, tol, soft);
                    run(&mut encoder, Pass::LeaveColor, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::ChangeToColor {
                    from_hue,
                    to_hue,
                    tol,
                    soft,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (from_hue, to_hue, tol, soft);
                    run(&mut encoder, Pass::ChangeToColor, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::ColorPass { key, sim, reverse } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (key[0], key[1], key[2], sim);
                    p.e = f32::from(reverse);
                    run(&mut encoder, Pass::ColorKeep, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::AlphaAdjust {
                    opacity,
                    ignore,
                    invert,
                    mask_only,
                } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b, p.c, p.d) = (
                        opacity,
                        f32::from(ignore),
                        f32::from(invert),
                        f32::from(mask_only),
                    );
                    run(&mut encoder, Pass::AlphaAdjust, p, cur, dst, None, None);
                    cur = dst;
                }
                Resolved::HFlip | Resolved::VFlip => {
                    let dst = (cur + 1) % 3;
                    let pass = if matches!(effect, Resolved::HFlip) {
                        Pass::HFlip
                    } else {
                        Pass::VFlip
                    };
                    run(&mut encoder, pass, Params::new(w, h), cur, dst, None, None);
                    cur = dst;
                }
                Resolved::Offset { shift, blend } => {
                    let dst = (cur + 1) % 3;
                    let mut p = Params::new(w, h);
                    (p.a, p.b) = offset_pixels(shift, w, h);
                    p.c = blend;
                    run(&mut encoder, Pass::OffsetFx, p, cur, dst, None, None);
                    cur = dst;
                }
            }
        }

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vfx_staging"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&bufs[cur], 0, &staging, 0, bytes);
        self.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| format!("falha ao aguardar a GPU: {e}"))?
            .map_err(|e| format!("falha ao mapear a saída da GPU: {e}"))?;
        let mapped = slice.get_mapped_range();
        let floats: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
        let out = Work {
            width: w as usize,
            height: h as usize,
            px: floats.to_vec(),
        };
        drop(mapped);
        staging.unmap();

        let mut result = frame.clone();
        out.write_to(&mut result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composite::GpuCompositor;
    use crate::generators::smpte_color_bars;
    use crate::vfx::{
        apply_chain_cpu, resolve_chain, VideoEffect, BLACK_AND_WHITE, BRIGHTNESS_CONTRAST,
        GAUSSIAN_BLUR, HSL_ADJUST, INVERT, LINEAR_BLUR, MEDIAN, MIRROR, PIXELATE, RADIAL_BLUR,
        SEPIA, SHARPEN, SWIRL, UNSHARP_MASK, VIGNETTE,
    };

    fn gpu() -> Option<VfxGpu> {
        let comp = GpuCompositor::try_headless()?;
        let (device, queue) = comp.device_queue();
        Some(VfxGpu::new(device, queue))
    }

    fn max_difference(a: &FrameBuffer, b: &FrameBuffer) -> u8 {
        a.rgba
            .iter()
            .zip(&b.rgba)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn the_gpu_chain_matches_the_cpu_reference() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect parity test skipped");
            return;
        };
        let chain = [
            VideoEffect::new(GAUSSIAN_BLUR)
                .with("h_range", 0.02)
                .with("v_range", 0.01),
            VideoEffect::new(SHARPEN),
            VideoEffect::new(UNSHARP_MASK).with("ratio", 1.5),
            VideoEffect::new(VIGNETTE).with("center_x", 0.3),
            VideoEffect::new(BRIGHTNESS_CONTRAST).with("contrast", 0.3),
            VideoEffect::new(PIXELATE).with("h_size", 0.02),
            VideoEffect::new(INVERT).with("amount", 0.5),
            VideoEffect::new(BLACK_AND_WHITE),
        ];
        let original = smpte_color_bars(320, 180);
        let mut cpu = original.clone();
        apply_chain_cpu(&mut cpu, &chain);
        let on_gpu = gpu
            .process(&original, &resolve_chain(&chain))
            .expect("gpu chain");
        assert_ne!(on_gpu, original);
        let diff = max_difference(&cpu, &on_gpu);
        assert!(diff <= 2, "largest per-channel difference {diff}");
    }

    #[test]
    fn each_effect_alone_matches_on_a_picture_with_transparency() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect parity test skipped");
            return;
        };
        let mut original = smpte_color_bars(200, 120);
        for (i, p) in original.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if (i % 200) < 60 {
                p[3] = 90;
            }
        }
        for fx in [
            VideoEffect::new(GAUSSIAN_BLUR).with("h_range", 0.03),
            VideoEffect::new(SHARPEN),
            VideoEffect::new(UNSHARP_MASK),
            VideoEffect::new(VIGNETTE),
            VideoEffect::new(BLACK_AND_WHITE).with("amount", 0.7),
            VideoEffect::new(INVERT),
            VideoEffect::new(BRIGHTNESS_CONTRAST)
                .with("brightness", 0.1)
                .with("contrast", 0.4),
            VideoEffect::new(BRIGHTNESS_CONTRAST).with("contrast", -0.5),
            VideoEffect::new(PIXELATE)
                .with("h_size", 0.07)
                .with("v_size", 0.03),
            VideoEffect::new(MIRROR),
            VideoEffect::new(MIRROR)
                .with("angle", 35.0)
                .with("center_x", 0.4),
            VideoEffect::new(MIRROR)
                .with("angle", 270.0)
                .with("center_y", 0.7),
            VideoEffect::new(SEPIA),
            VideoEffect::new(SEPIA)
                .with("strength", 0.4)
                .with("blue", 0.6),
            VideoEffect::new(SWIRL),
            VideoEffect::new(SWIRL)
                .with("ratio", -1.2)
                .with("center_x", 0.3)
                .with("h_size", 0.8),
            VideoEffect::new(RADIAL_BLUR),
            VideoEffect::new(RADIAL_BLUR)
                .with("level", 0.8)
                .with("center_x", 0.2)
                .with("center_y", 0.7),
            VideoEffect::new(MEDIAN),
            VideoEffect::new(MEDIAN)
                .with("h_range", 0.004)
                .with("v_range", 0.001),
            VideoEffect::new(HSL_ADJUST).with("add_to_hue", 0.1),
            VideoEffect::new(HSL_ADJUST)
                .with("add_to_hue", 0.62)
                .with("saturation", 1.4)
                .with("luminosity", 0.8),
            VideoEffect::new(LINEAR_BLUR),
            VideoEffect::new(LINEAR_BLUR)
                .with("angle", 37.0)
                .with("ratio", 0.8),
            VideoEffect::new(crate::vfx::COLOR_BALANCE)
                .with("shadow_b", 0.6)
                .with("mid_r", -0.4)
                .with("hi_g", 0.8)
                .with("preserve", 1.0),
            VideoEffect::new(crate::vfx::COLOR_BALANCE).with("hi_r", 1.0),
            VideoEffect::new(crate::vfx::LEVELS)
                .with("in_black", 20.0)
                .with("in_white", 200.0)
                .with("gamma", 150.0)
                .with("out_black", 10.0)
                .with("out_white", 240.0),
            VideoEffect::new(crate::vfx::GAMMA_CORRECTION).with("gamma", 17.0),
            VideoEffect::new(crate::vfx::POSTERIZE).with("levels", 5.0),
            VideoEffect::new(crate::vfx::EXTRACT)
                .with("black", 60.0)
                .with("white", 190.0),
            VideoEffect::new(crate::vfx::EXTRACT)
                .with("black", 60.0)
                .with("white", 190.0)
                .with("softness", 40.0)
                .with("invert", 1.0),
            VideoEffect::new(crate::vfx::TINT)
                .with("white_g", 0.4)
                .with("white_b", 0.1),
            VideoEffect::new(crate::vfx::TINT)
                .with("black_b", 0.3)
                .with("amount", 0.6),
            VideoEffect::new(crate::vfx::CHANNEL_MIX)
                .with("rr", 0.5)
                .with("rg", 0.6)
                .with("gb", -0.8)
                .with("bc", 0.3),
            VideoEffect::new(crate::vfx::CHANNEL_MIX)
                .with("rb", 1.2)
                .with("monochrome", 1.0),
            VideoEffect::new(crate::vfx::ASC_CDL)
                .with("r_slope", 1.4)
                .with("g_offset", -0.1)
                .with("b_power", 1.8)
                .with("saturation", 0.7),
            VideoEffect::new(crate::vfx::LEAVE_COLOR)
                .with("amount", 0.9)
                .with("key_g", 0.7)
                .with("tolerance", 0.1)
                .with("softness", 0.2),
            VideoEffect::new(crate::vfx::CHANGE_TO_COLOR)
                .with("hue_tol", 0.1)
                .with("softness", 0.4),
            VideoEffect::new(crate::vfx::COLOR_PASS).with("similarity", 0.5),
            VideoEffect::new(crate::vfx::COLOR_PASS)
                .with("similarity", 0.5)
                .with("reverse", 1.0),
            VideoEffect::new(crate::vfx::ALPHA_ADJUST)
                .with("opacity", 0.6)
                .with("invert", 1.0),
            VideoEffect::new(crate::vfx::ALPHA_ADJUST).with("mask_only", 1.0),
            VideoEffect::new(crate::vfx::ALPHA_ADJUST).with("ignore", 1.0),
            VideoEffect::new(crate::vfx::HORIZONTAL_FLIP),
            VideoEffect::new(crate::vfx::VERTICAL_FLIP),
            VideoEffect::new(crate::vfx::OFFSET)
                .with("shift_x", 0.8)
                .with("shift_y", 0.3),
            VideoEffect::new(crate::vfx::OFFSET)
                .with("shift_x", 0.2)
                .with("blend", 0.4),
        ] {
            let chain = [fx];
            let mut cpu = original.clone();
            apply_chain_cpu(&mut cpu, &chain);
            let on_gpu = gpu
                .process(&original, &resolve_chain(&chain))
                .expect("gpu chain");
            let diff = max_difference(&cpu, &on_gpu);
            assert!(diff <= 2, "{}: difference {diff}", chain[0].kind);
        }
    }

    #[test]
    fn every_transition_matches_the_cpu_on_a_picture_with_transparency() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU transition parity test skipped");
            return;
        };
        let mut original = smpte_color_bars(160, 90);
        for (i, p) in original.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if (i % 160) < 30 {
                p[3] = 120;
            }
        }
        for spec in tr::transitions() {
            let t = tr::resolve(spec.id, &std::collections::BTreeMap::new()).expect("resolves");
            for progress in [0.0, 0.2, 0.55, 0.9] {
                let cpu = tr::apply(&original, &t, progress);
                let on_gpu = gpu
                    .process_transition(&original, &t, progress)
                    .expect("gpu transition");
                let diff = max_difference(&cpu, &on_gpu);
                assert!(diff <= 3, "{} at {progress}: difference {diff}", spec.id);
            }
        }
        // Values away from the defaults, the second mode of the split included
        let extras: [(&str, &[(&str, f32)]); 6] = [
            (
                tr::SPLIT,
                &[("mode", 1.0), ("center_x", 0.3), ("center_y", 0.7)],
            ),
            (
                tr::SPLIT,
                &[("center_x", 0.8), ("center_y", 0.2), ("softness", 0.3)],
            ),
            (tr::SQUEEZE, &[("angle", 90.0)]),
            (tr::SQUEEZE, &[("angle", 200.0)]),
            (
                tr::SPIRAL,
                &[("turns", 2.0), ("zoom", 0.3), ("clockwise", 0.0)],
            ),
            (tr::FLASH, &[("red", 0.2), ("blue", 0.6)]),
        ];
        for (id, values) in extras {
            let params = values.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
            let t = tr::resolve(id, &params).expect("resolves");
            for progress in [0.3, 0.7] {
                let cpu = tr::apply(&original, &t, progress);
                let on_gpu = gpu
                    .process_transition(&original, &t, progress)
                    .expect("gpu transition");
                let diff = max_difference(&cpu, &on_gpu);
                assert!(
                    diff <= 3,
                    "{id} {values:?} at {progress}: difference {diff}"
                );
            }
        }
        // The end of a transition is the layer untouched
        let t = tr::resolve(tr::IRIS, &std::collections::BTreeMap::new()).unwrap();
        assert_eq!(
            gpu.process_transition(&original, &t, 1.0).unwrap(),
            original
        );
    }

    #[test]
    fn the_median_matches_on_a_picture_tall_enough_for_a_window() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU median test skipped");
            return;
        };
        // 600 pixels tall: the largest range is two pixels on each side
        let mut original = smpte_color_bars(640, 600);
        for (i, p) in original.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if i % 7 == 0 {
                *p = [255, 255, 255, 255];
            }
            if (i % 640) < 40 {
                p[3] = 100;
            }
        }
        let chain = [VideoEffect::new(MEDIAN)
            .with("h_range", 0.004)
            .with("v_range", 0.002)];
        let mut cpu = original.clone();
        apply_chain_cpu(&mut cpu, &chain);
        let on_gpu = gpu
            .process(&original, &resolve_chain(&chain))
            .expect("gpu chain");
        assert_ne!(cpu, original);
        assert!(max_difference(&cpu, &on_gpu) <= 1);
    }

    /// A smooth picture (no hard edges, so the two sides can differ by rounding only).
    fn smooth_picture(w: u32, h: u32, flip: bool) -> FrameBuffer {
        let mut f = FrameBuffer {
            width: w,
            height: h,
            rgba: vec![0; (w * h * 4) as usize],
        };
        for y in 0..h {
            for x in 0..w {
                let r = (x * 255 / (w - 1)) as u8;
                let g = (y * 255 / (h - 1)) as u8;
                let i = ((y * w + x) * 4) as usize;
                f.rgba[i] = if flip { 255 - r } else { r };
                f.rgba[i + 1] = g;
                f.rgba[i + 2] = if flip { 255 - g } else { 128 };
                f.rgba[i + 3] = if x < 12 { 200 } else { 255 };
            }
        }
        f
    }

    #[test]
    fn the_transitions_that_see_the_clip_before_match_the_cpu_on_every_side() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU two-layer transition test skipped");
            return;
        };
        let outgoing = smooth_picture(160, 90, true);
        let incoming = smooth_picture(160, 90, false);
        let mut cases: Vec<(&str, Vec<(&str, f32)>)> = Vec::new();
        for angle in [0.0, 90.0, 180.0, 270.0] {
            cases.push((tr::PUSH, vec![("angle", angle)]));
            cases.push((
                tr::PUSH,
                vec![
                    ("angle", angle),
                    ("push_off", 0.0),
                    ("border", 0.1),
                    ("border_feather", 0.5),
                    ("border_g", 0.2),
                ],
            ));
            cases.push((tr::PAGE_ROLL, vec![("angle", angle), ("radius", 0.1)]));
            cases.push((tr::PAGE_PEEL, vec![("angle", angle)]));
        }
        for (kind, values) in cases {
            let params = values.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
            let t = tr::resolve(kind, &params).expect("resolves");
            for progress in [0.2, 0.5, 0.8] {
                let cpu = tr::apply2(Some(&outgoing), &incoming, &t, progress);
                let on_gpu = gpu
                    .process_transition2(Some(&outgoing), &incoming, &t, progress)
                    .expect("gpu transition");
                let diff = max_difference(&cpu, &on_gpu);
                assert!(
                    diff <= 3,
                    "{kind} {values:?} at {progress}: difference {diff}"
                );
            }
        }
    }

    #[test]
    fn an_empty_chain_returns_the_frame_and_a_huge_frame_is_refused() {
        let Some(gpu) = gpu() else {
            eprintln!("no graphics adapter: GPU effect test skipped");
            return;
        };
        let frame = smpte_color_bars(64, 36);
        assert_eq!(gpu.process(&frame, &[]).unwrap(), frame);
        let limit = u64::from(gpu.device.limits().max_storage_buffer_binding_size);
        let side = ((limit / 16) as f64).sqrt() as u32 + 64;
        let big = FrameBuffer {
            width: side,
            height: side,
            rgba: Vec::new(),
        };
        let chain = resolve_chain(&[VideoEffect::new(SHARPEN)]);
        assert!(gpu.process(&big, &chain).is_err());
    }
}
