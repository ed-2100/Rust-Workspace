#version 450

layout(location = 0) in vec2 position;
layout(location = 0) out vec4 fragColor;

#define NUM_POINTS 4

layout(set = 0, binding = 0) uniform Position {
    vec2 points_position[NUM_POINTS];
};
layout(set = 0, binding = 1) uniform Color {
    vec3 points_color[NUM_POINTS];
};

void main() {
    vec3 sum_color = {0, 0, 0};
    for (int i = 0; i < NUM_POINTS; i++) {
        vec2 delta = position - points_position[i];
        float dist_sqrd = dot(delta, delta);
        sum_color += points_color[i] * (1 / (dist_sqrd * 300 + 1));
    }
    sum_color = clamp(sum_color, 0, 1);

    fragColor = vec4(sum_color, 1.0);
}
