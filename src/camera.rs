use glam::Vec3;
use uefi::proto::console::text::Key;
use libm::{cosf, sinf};

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,      // Horizontal angle
    pub pitch: f32,    // Vertical angle
    pub up: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec3::new(1.5, 1.5, 5.0),
            yaw: -core::f32::consts::FRAC_PI_2, // Look towards negative Z
            pitch: 0.0,
            up: Vec3::Y,
        }
    }

    pub fn handle_input(&mut self, key: Key) {
        if let Key::Printable(c) = key {
            let move_speed = 0.2;
            let rotate_speed = 0.05;

            // Calculate forward and right vectors for movement
            let forward = Vec3::new(cosf(self.yaw), 0.0, sinf(self.yaw)).normalize();
            let right = forward.cross(self.up).normalize();

            match u16::from(c) as u8 as char {
                // Movement
                'w' => self.position += forward * move_speed,
                's' => self.position -= forward * move_speed,
                'a' => self.position -= right * move_speed,
                'd' => self.position += right * move_speed,

                // Rotation
                'j' => self.yaw -= rotate_speed,
                'l' => self.yaw += rotate_speed,
                _ => {},
            }
        }
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        let look_direction = Vec3::new(
            cosf(self.yaw) * cosf(self.pitch),
            sinf(self.pitch),
            sinf(self.yaw) * cosf(self.pitch),
        ).normalize();
        let target = self.position + look_direction;

        glam::Mat4::look_at_rh(self.position, target, self.up)
    }
}
