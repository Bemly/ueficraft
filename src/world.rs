use glam::Vec3;
use uefi::proto::console::gop::BltPixel;

// Cube vertices
pub const CUBE_VERTICES: [Vec3; 8] = [
    Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, -0.5, -0.5),
    Vec3::new(0.5, 0.5, -0.5), Vec3::new(-0.5, 0.5, -0.5),
    Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, -0.5, 0.5),
    Vec3::new(0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, 0.5),
];

// Indices for 12 triangles (6 faces), in counter-clockwise order from the outside
pub const CUBE_INDICES: [usize; 36] = [
    0, 1, 2, 0, 2, 3, // Front
    4, 7, 6, 4, 6, 5, // Back
    4, 0, 3, 4, 3, 7, // Left
    1, 5, 6, 1, 6, 2, // Right
    3, 2, 6, 3, 6, 7, // Top
    0, 4, 5, 0, 5, 1, // Bottom
];

// Colors for each of the 6 faces
pub const FACE_COLORS: [BltPixel; 6] = [
    BltPixel::new(255, 0, 0),   // Front - Red
    BltPixel::new(0, 255, 0),   // Back - Green
    BltPixel::new(0, 0, 255),   // Left - Blue
    BltPixel::new(255, 255, 0), // Right - Yellow
    BltPixel::new(255, 0, 255), // Top - Magenta
    BltPixel::new(0, 255, 255), // Bottom - Cyan
];

// Normals for each of the 6 faces
pub const FACE_NORMALS: [Vec3; 6] = [
    Vec3::new(0.0, 0.0, -1.0), // Front
    Vec3::new(0.0, 0.0, 1.0),  // Back
    Vec3::new(-1.0, 0.0, 0.0), // Left
    Vec3::new(1.0, 0.0, 0.0),  // Right
    Vec3::new(0.0, 1.0, 0.0),  // Top
    Vec3::new(0.0, -1.0, 0.0), // Bottom
];

// A simple directional light
pub const LIGHT_DIRECTION: Vec3 = Vec3::new(-0.577, -0.577, -0.577); // Normalized vector

// World definition
pub const WORLD_SIZE: usize = 3;
pub const WORLD: [[[u8; WORLD_SIZE]; WORLD_SIZE]; WORLD_SIZE] = [
    [[1, 0, 1], [0, 1, 0], [1, 0, 1]],
    [[0, 1, 0], [1, 0, 1], [0, 1, 0]],
    [[1, 0, 1], [0, 1, 0], [1, 0, 1]],
];
