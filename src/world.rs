use alloc::vec::Vec;
use alloc::vec;
use glam::{IVec3, Vec3};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOL: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Block {
    Air = 0,
    Stone = 1,
    Grass = 2,
    Dirt = 3,
    Bedrock = 4,
}

impl Block {
    #[inline]
    pub fn is_air(&self) -> bool {
        *self == Block::Air
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        !self.is_air()
    }
}

pub struct Chunk {
    pub position: IVec3,
    pub blocks: [Block; CHUNK_VOL],
    pub compressed_data: Vec<u8>,
}

impl Chunk {
    pub fn new(position: IVec3) -> Self {
        Self {
            position,
            blocks: [Block::Air; CHUNK_VOL],
            compressed_data: Vec::new(),
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Block {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return Block::Air;
        }
        self.blocks[x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: Block) {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            return;
        }
        self.blocks[x + y * CHUNK_SIZE + z * CHUNK_SIZE * CHUNK_SIZE] = block;
    }

    pub fn compress(&mut self) {
        let mut rle = Vec::new();
        if CHUNK_VOL == 0 { return; }

        let mut last = self.blocks[0];
        let mut count = 0u16;

        for &b in &self.blocks {
            if b == last && count < u16::MAX {
                count += 1;
            } else {
                rle.push(last as u8);
                rle.extend_from_slice(&count.to_le_bytes());
                last = b;
                count = 1;
            }
        }
        rle.push(last as u8);
        rle.extend_from_slice(&count.to_le_bytes());
        self.compressed_data = rle;
    }

    // Optimized Greedy Meshing with Bitwise Binary Culling and LOD
    pub fn generate_mesh(&self, lod: usize) -> Vec<Face> {
        let mut faces = Vec::new();
        let step = 1 << lod;
        let size = CHUNK_SIZE >> lod;

        // Iterate 3 axes
        for axis in 0..3 {
            let u_axis = (axis + 1) % 3;
            let v_axis = (axis + 2) % 3;

            let mut plane_prev = vec![0u32; size];
            let mut plane_curr = vec![0u32; size];

            // Iterate depth d from 0 to size (inclusive for boundary check)
            for d in 0..=size {
                // Build plane_curr for depth `d`
                if d < size {
                    for v in 0..size {
                        let mut row_bits = 0u32;
                        for u in 0..size {
                            let (x, y, z) = match axis {
                                0 => (d, u, v),
                                1 => (u, d, v),
                                2 => (u, v, d),
                                _ => (0,0,0),
                            };

                            let blk = self.get(x * step, y * step, z * step);
                            if !blk.is_air() {
                                row_bits |= 1 << u;
                            }
                        }
                        plane_curr[v] = row_bits;
                    }
                } else {
                    plane_curr.fill(0);
                }

                // Check faces between plane_prev (d-1) and plane_curr (d)
                for dir in 0..2 {
                    let is_pos = dir == 0; // +Axis (Normal points to +)

                    let mut mask = vec![0u32; size];
                    for v in 0..size {
                        mask[v] = if is_pos {
                            plane_prev[v] & !plane_curr[v]
                        } else {
                            !plane_prev[v] & plane_curr[v]
                        };
                    }

                    // Greedy Mesh
                    for v in 0..size {
                        let mut u = 0;
                        while u < size {
                            if (mask[v] >> u) & 1 == 1 {
                                // Found a face start
                                let d_blk = if is_pos { d.wrapping_sub(1) } else { d };

                                let (bx, by, bz) = match axis {
                                    0 => (d_blk, u, v),
                                    1 => (u, d_blk, v),
                                    2 => (u, v, d_blk),
                                    _ => (0,0,0),
                                };
                                let blk = self.get(bx * step, by * step, bz * step);

                                // Compute width
                                let mut w = 1;
                                while u + w < size && ((mask[v] >> (u + w)) & 1 == 1) {
                                    let (nx, ny, nz) = match axis {
                                        0 => (d_blk, u + w, v),
                                        1 => (u + w, d_blk, v),
                                        2 => (u + w, v, d_blk),
                                        _ => (0,0,0),
                                    };
                                    if self.get(nx * step, ny * step, nz * step) != blk {
                                        break;
                                    }
                                    w += 1;
                                }

                                // Compute height
                                let mut h = 1;
                                'h_loop: while v + h < size {
                                    let row_bits = mask[v + h];
                                    let row_mask = ((1 << w) - 1) << u;
                                    if (row_bits & row_mask) != row_mask {
                                        break;
                                    }

                                    for k in 0..w {
                                        let (nx, ny, nz) = match axis {
                                            0 => (d_blk, u + k, v + h),
                                            1 => (u + k, d_blk, v + h),
                                            2 => (u + k, v + h, d_blk),
                                            _ => (0,0,0),
                                        };
                                        if self.get(nx * step, ny * step, nz * step) != blk {
                                            break 'h_loop;
                                        }
                                    }
                                    h += 1;
                                }

                                // Add Face
                                let pos_scale = step as i32;
                                let final_pos = match axis {
                                    0 => IVec3::new(d as i32, u as i32, v as i32),
                                    1 => IVec3::new(u as i32, d as i32, v as i32),
                                    2 => IVec3::new(u as i32, v as i32, d as i32),
                                    _ => IVec3::ZERO,
                                } * pos_scale + self.position * (CHUNK_SIZE as i32);

                                let mut du = [0; 3]; du[u_axis] = w as i32;
                                let mut dv = [0; 3]; dv[v_axis] = h as i32;
                                let final_size = IVec3::new(du[0] + dv[0], du[1] + dv[1], du[2] + dv[2]) * pos_scale;

                                let face_axis = if is_pos {
                                    axis as u8 * 2 + 1
                                } else {
                                    axis as u8 * 2
                                };

                                faces.push(Face {
                                    pos: final_pos,
                                    size: final_size,
                                    axis: face_axis,
                                    block: blk,
                                });

                                // Clear bits
                                let row_mask = !(((1 << w) - 1) << u);
                                for l in 0..h {
                                    mask[v + l] &= row_mask;
                                }

                                u += w;
                            } else {
                                u += 1;
                            }
                        }
                    }
                }

                plane_prev.copy_from_slice(&plane_curr);
            }
        }

        faces
    }
}

#[derive(Clone, Copy)]
pub struct Face {
    pub pos: IVec3,
    pub size: IVec3,
    pub axis: u8, // 0: -X, 1: +X, 2: -Y, 3: +Y, 4: -Z, 5: +Z
    pub block: Block,
}
