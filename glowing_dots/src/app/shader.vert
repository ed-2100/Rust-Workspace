#version 450

layout(location = 0) in vec2 position;
layout(location = 0) out vec2 fragPosition;

void main() {
    fragPosition = position;

    // Convert clip coordinates to NDC by using
    // 1.0 for the perspective divisor.
    gl_Position = vec4(position, 0.0, 1.0);
}
