#version 460

const vec2 FS_QUAD_VERTICES[4] = {
    vec2(-1.0, 1.0),  // Top left
    vec2(-1.0, -1.0), // Bottom left
    vec2(1.0, -1.0),  // Bottom right
    vec2(1.0, 1.0)    // Top right
};

void main() {
    vec2 vertex;
    
    switch (gl_VertexIndex) {
    case 0:
    case 3:
        vertex = FS_QUAD_VERTICES[0];
        break;
    
    case 1:
        vertex = FS_QUAD_VERTICES[1];
        break;

    case 2:
    case 4:
        vertex = FS_QUAD_VERTICES[2];
        break;

    case 5:
        vertex = FS_QUAD_VERTICES[3];
    }

    gl_Position = vec4(vertex, 0.0, 1.0);
}
