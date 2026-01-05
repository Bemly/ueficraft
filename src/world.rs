use alloc::vec::Vec;
use glam::{Vec3, vec3};

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub id: u8,
}

impl Block {
    pub fn new(id: u8) -> Self {
        Self { id }
    }
}

pub struct World {
    pub blocks: Vec<(Vec3, Block)>,
}

impl World {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn add_block(&mut self, pos: Vec3, block: Block) {
        self.blocks.push((pos, block));
    }

    pub fn generate_example(&mut self) {
        // Create a 3x3 floor
        for x in -1..=1 {
            for z in -1..=1 {
                self.add_block(vec3(x as f32, 0.0, z as f32), Block::new(1));
            }
        }
        // Add a block on top
        self.add_block(vec3(0.0, 1.0, 0.0), Block::new(2));
    }
}
