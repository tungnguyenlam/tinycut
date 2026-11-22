struct Params {
    width: u32,
    height: u32,
    style: u32,     // 0 = Gaussian, 1 = Pixelate, 2 = SolidMask
    box_count: u32,
};

struct RedactBox {
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> src_pixels: array<u32>;
@group(0) @binding(2) var<storage, read_write> dst_pixels: array<u32>;
@group(0) @binding(3) var<storage, read> boxes: array<RedactBox>;

fn unpack_rgba(p: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(p & 255u),
        f32((p >> 8u) & 255u),
        f32((p >> 16u) & 255u),
        f32((p >> 24u) & 255u),
    );
}

fn pack_rgba(c: vec4<f32>) -> u32 {
    let r = u32(clamp(round(c.x), 0.0, 255.0));
    let g = u32(clamp(round(c.y), 0.0, 255.0));
    let b = u32(clamp(round(c.z), 0.0, 255.0));
    let a = u32(clamp(round(c.w), 0.0, 255.0));
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= params.width || y >= params.height) {
        return;
    }

    let idx = y * params.width + x;
    var inside = false;
    for (var i = 0u; i < params.box_count; i = i + 1u) {
        let b = boxes[i];
        if (x >= b.x1 && x < b.x2 && y >= b.y1 && y < b.y2) {
            inside = true;
            break;
        }
    }

    if (!inside) {
        dst_pixels[idx] = src_pixels[idx];
        return;
    }

    if (params.style == 2u) {
        // SolidMask (opaque dark charcoal RGBA = 18, 18, 22, 255)
        dst_pixels[idx] = 0xFF161212u;
        return;
    }

    if (params.style == 1u) {
        // Pixelate (12x12 mosaic blocks)
        let block = 12u;
        let bx = min((x / block) * block + (block / 2u), params.width - 1u);
        let by = min((y / block) * block + (block / 2u), params.height - 1u);
        dst_pixels[idx] = src_pixels[by * params.width + bx];
        return;
    }

    // Gaussian-approximated box blur (radius = 4, stride = 1)
    var acc = vec4<f32>(0.0);
    var count = 0.0;
    let radius = 4;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        let sy = u32(clamp(i32(y) + dy, 0, i32(params.height) - 1));
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let sx = u32(clamp(i32(x) + dx, 0, i32(params.width) - 1));
            acc = acc + unpack_rgba(src_pixels[sy * params.width + sx]);
            count = count + 1.0;
        }
    }
    dst_pixels[idx] = pack_rgba(acc / max(count, 1.0));
}
