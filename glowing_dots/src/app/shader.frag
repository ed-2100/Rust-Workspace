#version 450

layout(location = 0) in vec2 position;
layout(location = 0) out vec4 fragColor;

layout(set = 0, binding = 0) uniform Position {
    vec2 points_position[4];
};

vec3 points_color[4] = {
    vec3(1.0, 1.0, 1.0),
    vec3(1.0, 0.0, 0.0),
    vec3(0.0, 1.0, 0.0),
    vec3(0.0, 0.0, 1.0)
};

void main() {
    vec3 sum_color = {0, 0, 0};
    for (int i = 0; i < 4; i++) {
        vec2 delta = position - points_position[i];
        float dist_sqrd = dot(delta, delta);
        sum_color += points_color[i] * (1 / (dist_sqrd * 300 + 1));
    }
    sum_color = clamp(sum_color, 0, 1);

    fragColor = vec4(sum_color, 1.0);
}
