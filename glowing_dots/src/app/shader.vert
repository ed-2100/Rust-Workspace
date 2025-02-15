#version 450

layout(location = 0) out vec2 fragPosition;

vec2 points[4] = {
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2(-1.0,  1.0),
};

void main() {
    fragPosition = points[gl_VertexIndex];

    // Convert clip coordinates to NDC by using
    // 1.0 for the perspective divisor.
    gl_Position = vec4(points[gl_VertexIndex], 0.0, 1.0);
}
