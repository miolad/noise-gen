#version 460

layout(std430, push_constant) readonly restrict uniform PushConstants {
    float frame_number;
    
    // Text bounding box
    float bb_min_x;
    float bb_min_y;
    float bb_max_x;
    float bb_max_y;
};

layout(location = 0) out vec4 out_color;

// From https://stackoverflow.com/questions/4200224/random-noise-functions-for-glsl
float rand(vec2 seed) {
    return fract(sin(dot(seed, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    if (gl_FragCoord.x > bb_min_x && gl_FragCoord.x < bb_max_x &&
        gl_FragCoord.y > bb_min_y && gl_FragCoord.y < bb_max_y) {

        out_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    
    vec2 seed = mod(gl_FragCoord.xy * frame_number, vec2(7.312, 39.239));
    float r = rand(seed);
    float g = rand(seed * r);
    float b = rand(seed * g);

    out_color = vec4(r, g, b, 1.0);
}
