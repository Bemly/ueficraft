use alloc::vec;
use alloc::vec::Vec;

// SVO 深度为8, 给予我们一个 256x256x256 的世界
pub const SVO_DEPTH: u32 = 8;
pub const WORLD_SIZE: i32 = 1 << SVO_DEPTH;

// 节点指针和方块类型
type NodePtr = u32;
type BlockType = u8;

// 用最高位作为标记，判断一个节点是叶子节点（包含方块数据）还是枝干节点（指向下一层）
const LEAF_FLAG: u32 = 1 << 31;

/// 使用稀疏体素八叉树 (SVO) 来存储世界数据
pub struct Svo {
    /// 节点池。每8个 `u32` 元素代表一个节点的8个子节点。
    node_pool: Vec<u32>,
}

// 全局唯一的 SVO 世界实例
static mut SVO_WORLD: Option<Svo> = None;

impl Svo {
    pub fn new() -> Self {
        // 初始化根节点的8个子节点，全部为空
        let node_pool = vec![0; 8];
        Self { node_pool }
    }

    /// 根据坐标获取方块类型
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if x < 0 || y < 0 || z < 0 || x >= WORLD_SIZE || y >= WORLD_SIZE || z >= WORLD_SIZE {
            return 0; // 世界之外是空气
        }

        let mut current_node_children_ptr: NodePtr = 0; // 从根节点开始

        for level in (1..=SVO_DEPTH).rev() {
            let level_size = 1 << (level - 1);
            
            // 根据坐标在该层级的位置，确定它属于8个子节点中的哪一个
            // 这等价于使用 Morton 码的位信息进行寻址
            let child_index_in_node = ((((x / level_size) & 1) as u32) << 0) |
                                      ((((y / level_size) & 1) as u32) << 1) |
                                      ((((z / level_size) & 1) as u32) << 2);

            let child_entry_index = (current_node_children_ptr + child_index_in_node) as usize;
            let child_entry_value = self.node_pool[child_entry_index];

            if child_entry_value == 0 {
                return 0; // 空间为空
            }

            if (child_entry_value & LEAF_FLAG) != 0 {
                // 这是一个叶子节点，低位存储着方块类型
                return (child_entry_value & !LEAF_FLAG) as BlockType;
            }

            // 这是一个枝干节点，其值是指向下一层子节点的指针
            current_node_children_ptr = child_entry_value;
        }

        0 // 不应到达此处，代表非叶子节点的内部是空的
    }

    /// 插入一个方块，可能会替换现有方块或整个子树
    pub fn insert(&mut self, x: i32, y: i32, z: i32, block_type: BlockType) {
        if x < 0 || y < 0 || z < 0 || x >= WORLD_SIZE || y >= WORLD_SIZE || z >= WORLD_SIZE || block_type == 0 {
            return; // 不插入空气或界外方块
        }

        let mut current_node_children_ptr: NodePtr = 0;

        for level in (1..=SVO_DEPTH).rev() {
            let level_size = 1 << (level - 1);
            let child_index_in_node = ((((x / level_size) & 1) as u32) << 0) |
                                      ((((y / level_size) & 1) as u32) << 1) |
                                      ((((z / level_size) & 1) as u32) << 2);

            let child_entry_index = (current_node_children_ptr + child_index_in_node) as usize;

            if level == 1 {
                // 到达最底层，直接插入叶子节点
                self.node_pool[child_entry_index] = (block_type as u32) | LEAF_FLAG;
                return;
            }

            let child_entry_value = self.node_pool[child_entry_index];

            if child_entry_value == 0 {
                // 路径不存在，创建新的枝干节点
                let new_node_children_ptr = self.node_pool.len() as NodePtr;
                self.node_pool.extend_from_slice(&[0; 8]); // 分配8个新的子节点空间
                self.node_pool[child_entry_index] = new_node_children_ptr;
                current_node_children_ptr = new_node_children_ptr;
            } 
            else if (child_entry_value & LEAF_FLAG) != 0 {
                // 路径上是一个叶子节点，需要将其细分
                let old_block_type = (child_entry_value & !LEAF_FLAG) as BlockType;
                if old_block_type == block_type { return; } // 类型相同，无需操作

                let new_node_children_ptr = self.node_pool.len() as NodePtr;
                // 创建新的节点块，并用旧的方块类型填充它
                let new_children = [(old_block_type as u32) | LEAF_FLAG; 8];
                self.node_pool.extend_from_slice(&new_children);
                
                self.node_pool[child_entry_index] = new_node_children_ptr;
                current_node_children_ptr = new_node_children_ptr;
            } 
            else {
                // 路径上是已存在的枝干节点，继续向下遍历
                current_node_children_ptr = child_entry_value;
            }
        }
    }
}

/// 初始化世界
pub fn init_world() {
    let mut svo = Svo::new();
    let size = 32;

    // 创建一个y=0的地面
    for x in 0..size {
        for z in 0..size {
            svo.insert(x, 0, z, 1); // 石头地面
        }
    }

    // 创建一些墙
    for x in 0..size {
        svo.insert(x, 1, 0, 2); // 墙
        svo.insert(x, 2, 0, 2);
        svo.insert(0, 1, x, 2);
        svo.insert(0, 2, x, 2);
    }

    // 创建一个柱子
    for y in 1..5 {
        svo.insert(10, y, 10, 3); // 柱子
    }

    unsafe { SVO_WORLD = Some(svo); }
}

/// 公共API，从全局世界中获取方块
pub fn get_block(x: i32, y: i32, z: i32) -> u8 {
    if let Some(svo) = unsafe { (*(&raw const SVO_WORLD)).as_ref() } {
        svo.get_block(x, y, z)
    } else {
        0
    }
}
