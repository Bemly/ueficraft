use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive, ScopedProtocol};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use crate::ascii_font::FONT_8X16;
use crate::error::{Result, OK};
use crate::t;

pub struct Screen {
    gop: ScopedProtocol<GraphicsOutput>,
    row_ptr: usize,
}

impl Screen {
    pub fn new() -> Result<Self> {
        let handle = t!(get_handle_for_protocol::<GraphicsOutput>());
        let mut gop = t!(open_protocol_exclusive::<GraphicsOutput>(handle));
        Ok(Self { gop, row_ptr: 0 })
    }

    pub fn get_gop(&mut self) -> &mut ScopedProtocol<GraphicsOutput> { &mut self.gop }

    pub fn clear(&mut self) -> Result {
        let info = self.gop.current_mode_info();
        let (width, height) = info.resolution();

        t!(self.gop.blt(BltOp::VideoFill {
            color: BltPixel::new(0, 0, 0),
            dest: (0, 0),
            dims: (width, height),
        }));

        OK
    }

    pub fn println(&mut self, text: &str) {
        let mut x = 0;
        let (width, height) = self.gop.current_mode_info().resolution();

        let fg = BltPixel::new(255, 255, 255);
        let bg = BltPixel::new(0, 0, 0);

        if self.row_ptr + 20 >= height { self.row_ptr = 0 }

        for c in text.chars() {
            if c == '\n' {
                x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
                continue;
            }

            if x + 8 > width {
                x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
            }

            let index = (c as usize) & 0x7F;
            let glyph = &FONT_8X16[index];

            for row in 0..16 {
                let row_bits = glyph[row];
                for col in 0..8 {
                    let is_fg = (row_bits >> (7 - col)) & 1 == 1;
                    let color = if is_fg { fg } else { bg };

                    let _ = self.gop.blt(BltOp::VideoFill {
                        color,
                        dest: (x + col, self.row_ptr + row),
                        dims: (1, 1),
                    });
                }
            }
            x += 8;
        }
        self.row_ptr += 18;
    }
}