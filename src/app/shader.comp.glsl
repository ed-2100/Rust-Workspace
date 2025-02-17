#version 450

layout(local_size_x = 8, local_size_y = 8) in;

layout(set = 0, binding = 0) uniform Position {
    vec2 points_position[4];
};
layout(set = 0, binding = 1, rgba8) uniform writeonly image2D output_texture;

vec3 points_color[4] = {
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 0.0, 0.0),
    vec3(0.0, 1.0, 0.0),
    vec3(0.0, 0.0, 1.0)
};

const float bayer8x8[64] = float[](
     0, 48, 12, 60,  3, 51, 15, 63,
    32, 16, 44, 28, 35, 19, 47, 31,
     8, 56,  4, 52, 11, 59,  7, 55,
    40, 24, 36, 20, 43, 27, 39, 23,
     2, 50, 14, 62,  1, 49, 13, 61,
    34, 18, 46, 30, 33, 17, 45, 29,
    10, 58,  6, 54,  9, 57,  5, 53,
    42, 26, 38, 22, 41, 25, 37, 21
);

float getBayerValue(uvec2 coord) {
    uint x = coord.x % 8;
    uint y = coord.y % 8;
    return bayer8x8[y * 8 + x] / 64.0;
}

void main() {
    uvec2 pixel_coords = gl_GlobalInvocationID.xy;
    uvec2 texture_size = imageSize(output_texture);

    if (pixel_coords.x >= texture_size.x || pixel_coords.y >= texture_size.y) {
        return;
    }

    uint min_width = min(texture_size.x, texture_size.y);

    ivec2 centered_coords = ivec2(pixel_coords) - ivec2(texture_size / 2);

    vec2 ndc_coords = vec2(centered_coords) / float(min_width) * 2;
    ndc_coords.y = -ndc_coords.y;

    vec3 sum_color = vec3(0.0, 0.0, 0.0);
    for (int i = 0; i < 4; i++) {
        vec2 delta = ndc_coords - points_position[i];
        float dist_sqrd = dot(delta, delta);
        sum_color += points_color[i] / (dist_sqrd * 300.0 + 1.0);
    }

    sum_color = pow(sum_color, vec3(1.0 / 2.2)); // Gamma correction
    sum_color = sum_color + (getBayerValue(pixel_coords) - 0.5) * (1.0 / 255.0); // Bayer dithering

    imageStore(output_texture, ivec2(pixel_coords), vec4(sum_color, 1.0));
}
