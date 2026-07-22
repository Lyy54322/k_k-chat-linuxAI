//! Linux fbdev 帧缓冲驱动
//!
//! mmap /dev/fb0 显存，实现抗锯齿白色笔迹渲染。
//! 画布区域为屏幕下半 45%，与上方文字区显存物理隔离。

// ── Linux framebuffer 结构体（libc crate 不包含） ─────────────────
#[repr(C)]
pub struct fb_fix_screeninfo {
    pub id:           [u8; 16],
    pub smem_start:   u64,
    pub smem_len:     u32,
    pub type_:        u32,
    pub type_aux:     u32,
    pub visual:       u32,
    pub xpanstep:     u16,
    pub ypanstep:     u16,
    pub ywrapstep:    u16,
    pub line_length:  u32,
    pub mmio_start:   u64,
    pub mmio_len:     u32,
    pub accel:        u32,
    pub capabilities: u16,
    pub reserved:      [u16; 2],
}

#[repr(C)]
pub struct fb_bitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

#[repr(C)]
pub struct fb_var_screeninfo {
    pub xres:           u32,
    pub yres:           u32,
    pub xres_virtual:   u32,
    pub yres_virtual:   u32,
    pub xoffset:        u32,
    pub yoffset:        u32,
    pub bits_per_pixel: u32,
    pub grayscale:      u32,
    pub red:    fb_bitfield,
    pub green:  fb_bitfield,
    pub blue:   fb_bitfield,
    pub transp: fb_bitfield,
    pub nonstd:     u32,
    pub activate:   u32,
    pub height:     u32,
    pub width:      u32,
    pub accel_flags:u32,
    pub pixclock:   u32,
    pub left_margin:  u32,
    pub right_margin: u32,
    pub upper_margin:  u32,
    pub lower_margin:  u32,
    pub hsync_len:  u32,
    pub vsync_len:  u32,
    pub sync:       u32,
    pub vmode:      u32,
    pub rotate:     u32,
    pub colorspace:  u32,
    pub reserved:    [u32; 4],
}

// ioctl 常量
const FBIOGET_FSCREENINFO: u64 = 0x4602;
const FBIOGET_VSCREENINFO: u64 = 0x4600;

// ── Framebuffer 设备 ───────────────────────────────────────────────
pub struct Framebuffer {
    fd: i32,
    ptr: *mut u8,
    pub width: u32,
    pub height: u32,
    line_len: u32,
    bpp: u32,
    pub canvas_top: u32,
    pub canvas_height: u32,
}

impl Framebuffer {
    pub fn open() -> Result<Framebuffer, String> {
        let fd = unsafe { libc::open(b"/dev/fb0\0".as_ptr() as *const i8, libc::O_RDWR) };
        if fd < 0 { return Err("无法打开 /dev/fb0".into()); }

        let mut finfo: fb_fix_screeninfo = unsafe { std::mem::zeroed() };
        let mut vinfo: fb_var_screeninfo = unsafe { std::mem::zeroed() };

        if unsafe { libc::ioctl(fd, FBIOGET_FSCREENINFO, &mut finfo) } < 0 {
            unsafe { libc::close(fd); }
            return Err("FBIOGET_FSCREENINFO 失败".into());
        }
        if unsafe { libc::ioctl(fd, FBIOGET_VSCREENINFO, &mut vinfo) } < 0 {
            unsafe { libc::close(fd); }
            return Err("FBIOGET_VSCREENINFO 失败".into());
        }

        let screen_size = finfo.smem_len as usize;
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(), screen_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED, fd, 0,
            )
        };
        if ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd); }
            return Err("mmap framebuffer 失败".into());
        }

        let width  = vinfo.xres;
        let height = vinfo.yres;
        let bpp    = vinfo.bits_per_pixel;
        let line_len = finfo.line_length;
        let canvas_top    = height * 55 / 100;
        let canvas_height = height - canvas_top;

        Ok(Framebuffer { fd, ptr: ptr as *mut u8, width, height, line_len, bpp, canvas_top, canvas_height })
    }

    pub fn set_pixel(&self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        if x >= self.width || y < self.canvas_top || y >= self.height { return; }
        let bytes = self.bpp / 8;
        let off = (y as usize * self.line_len as usize) + (x as usize * bytes as usize);
        unsafe {
            let p = self.ptr.add(off);
            if bytes == 4 {
                *p = b; *p.add(1) = g; *p.add(2) = r; *p.add(3) = 0;
            } else if bytes == 3 {
                *p = b; *p.add(1) = g; *p.add(2) = r;
            }
        }
    }

    /// Bresenham 抗锯齿白色线条
    pub fn draw_aa_line(&self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (mut x0, mut y0) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.draw_aa_point(x0, y0);
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }

    fn draw_aa_point(&self, x: i32, y: i32) {
        self.blend_pixel(x, y, 255, 255, 255, 255);
        self.blend_pixel(x - 1, y, 255, 255, 255, 85);
        self.blend_pixel(x + 1, y, 255, 255, 255, 85);
        self.blend_pixel(x, y - 1, 255, 255, 255, 85);
        self.blend_pixel(x, y + 1, 255, 255, 255, 85);
    }

    fn blend_pixel(&self, x: i32, y: i32, r: u8, g: u8, b: u8, alpha: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return; }
        if (y as u32) < self.canvas_top { return; }
        let bytes = self.bpp / 8;
        let off = (y as usize * self.line_len as usize) + (x as usize * bytes as usize);
        let a = alpha as u32;
        let inv = 255 - a;
        if bytes == 4 {
            unsafe {
                let p = self.ptr.add(off);
                *p             = (((b as u32) * a + (*p as u32) * inv) / 255) as u8;
                *p.add(1)     = (((g as u32) * a + (*p.add(1) as u32) * inv) / 255) as u8;
                *p.add(2)     = (((r as u32) * a + (*p.add(2) as u32) * inv) / 255) as u8;
            }
        }
    }

    /// 清空画布（仅下半区域，不影响上方聊天文字）
    pub fn clear_canvas(&self) {
        let bytes = self.bpp / 8;
        for y in self.canvas_top..self.height {
            for x in 0..self.width {
                let off = (y as usize * self.line_len as usize) + (x as usize * bytes as usize);
                unsafe {
                    let p = self.ptr.add(off);
                    *p = 0; *p.add(1) = 0; *p.add(2) = 0;
                    if bytes == 4 { *p.add(3) = 0; }
                }
            }
        }
    }

    /// 画分隔线（已移除，上下区域无感过渡）
    #[allow(dead_code)]
    pub fn draw_separator(&self) {
        // 无感模式：不画任何分隔线，聊天区与手写区无缝衔接
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, (self.line_len * self.height) as usize);
            libc::close(self.fd);
        }
    }
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}