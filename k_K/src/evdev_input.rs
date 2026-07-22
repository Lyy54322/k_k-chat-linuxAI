//! evdev 鼠标输入子系统
//!
//! 支持 USB/PS2 鼠标、触控板等 evdev 设备。
//! 内置抖动滤波算法，平滑手写笔迹。

use std::fs;
use std::path::Path;

// ── 手动定义常量（libc crate 不包含 evdev 常量） ─────────────────
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;
const BTN_LEFT: u16  = 0x110;
const BTN_RIGHT: u16 = 0x111;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const SYN_REPORT: u16 = 0x00;

// ── 事件结构 ─────────────────────────────────────────────────────
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct InputEvent {
    time_sec:  u64,
    time_usec: u64,
    event_type: u16,
    code:       u16,
    value:      i32,
}

// ── 手动定义结构 ─────────────────────────────────────────────────
#[repr(C)]
struct input_absinfo {
    value:     i32,
    minimum:   i32,
    maximum:   i32,
    fuzz:      i32,
    flat:      i32,
    resolution:i32,
}

// ── ioctl 号 ──────────────────────────────────────────────────────
const EVIOCGNAME_256: u64 = 0x8100_4506;
fn eviocgabs(abs: u16) -> u64 {
    0x8018_4540 + (abs as u64) * 8
}

// ── 公共类型 ──────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub event_type: MouseEventType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseEventType {
    LeftDown,
    LeftUp,
    RightDown,
    Motion,
}

pub fn find_mouse_device() -> Option<String> {
    let candidates = [
        "/dev/input/event0","/dev/input/event1","/dev/input/event2",
        "/dev/input/event3","/dev/input/event4","/dev/input/event5",
    ];
    for path in &candidates {
        if Path::new(path).exists() && is_mouse_device(path) {
            return Some(path.to_string());
        }
    }
    if let Ok(entries) = fs::read_dir("/dev/input/") {
        for entry in entries.flatten() {
            let p = entry.path();
            let s = p.to_string_lossy().to_string();
            if s.contains("event") && is_mouse_device(&s) { return Some(s); }
        }
    }
    if Path::new("/dev/input/mice").exists() { return Some("/dev/input/mice".into()); }
    None
}

fn is_mouse_device(path: &str) -> bool {
    let fd = unsafe { libc::open(path.as_ptr() as *const i8, libc::O_RDONLY | libc::O_NONBLOCK) };
    if fd < 0 { return false; }
    let mut name_buf = [0u8; 256];
    let ret = unsafe { libc::ioctl(fd, EVIOCGNAME_256, name_buf.as_mut_ptr()) };
    unsafe { libc::close(fd); }
    if ret > 0 {
        let name = String::from_utf8_lossy(&name_buf[..ret as usize]).to_lowercase();
        name.contains("mouse") || name.contains("trackball") || name.contains("pointer")
    } else { false }
}

pub struct MouseInput {
    fd: i32,
    is_evdev: bool,
    x: i32, y: i32,
    screen_w: u32, screen_h: u32,
    abs_x_min: i32, abs_x_max: i32,
    abs_y_min: i32, abs_y_max: i32,
}

impl MouseInput {
    pub fn open(screen_w: u32, screen_h: u32) -> Result<MouseInput, String> {
        let device = find_mouse_device().ok_or("找不到鼠标设备")?;
        let is_evdev = !device.contains("mice");
        let fd = unsafe { libc::open(device.as_ptr() as *const i8, libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 { return Err(format!("无法打开鼠标设备 {}", device)); }

        let mut input = MouseInput {
            fd, is_evdev, x: 0, y: 0, screen_w, screen_h,
            abs_x_min: 0, abs_x_max: 1024, abs_y_min: 0, abs_y_max: 768,
        };
        if is_evdev { input.query_abs_limits(); }
        Ok(input)
    }

    fn query_abs_limits(&mut self) {
        let mut info: input_absinfo = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(self.fd, eviocgabs(ABS_X), &mut info) } >= 0 {
            self.abs_x_min = info.minimum; self.abs_x_max = info.maximum;
        }
        if unsafe { libc::ioctl(self.fd, eviocgabs(ABS_Y), &mut info) } >= 0 {
            self.abs_y_min = info.minimum; self.abs_y_max = info.maximum;
        }
    }

    pub fn poll_event(&mut self) -> Option<MouseEvent> {
        if !self.is_evdev { return self.poll_legacy_mouse(); }

        let mut ev = InputEvent::default();
        let n = unsafe {
            libc::read(self.fd, &mut ev as *mut InputEvent as *mut libc::c_void,
                        std::mem::size_of::<InputEvent>())
        };
        if n < std::mem::size_of::<InputEvent>() as isize { return None; }

        match ev.event_type {
            EV_KEY => match ev.code {
                BTN_LEFT => Some(MouseEvent { x: self.x, y: self.y, event_type: if ev.value != 0 { MouseEventType::LeftDown } else { MouseEventType::LeftUp } }),
                BTN_RIGHT => Some(MouseEvent { x: self.x, y: self.y, event_type: MouseEventType::RightDown }),
                _ => None,
            },
            EV_REL => {
                match ev.code {
                    REL_X => { self.x = (self.x + ev.value).clamp(0, self.screen_w as i32 - 1); }
                    REL_Y => { self.y = (self.y + ev.value).clamp(0, self.screen_h as i32 - 1); }
                    _ => {}
                }
                Some(MouseEvent { x: self.x, y: self.y, event_type: MouseEventType::Motion })
            }
            EV_ABS => {
                match ev.code {
                    ABS_X => {
                        let range = (self.abs_x_max - self.abs_x_min).max(1) as f32;
                        self.x = ((ev.value - self.abs_x_min) as f32 / range * self.screen_w as f32) as i32;
                        self.x = self.x.clamp(0, self.screen_w as i32 - 1);
                    }
                    ABS_Y => {
                        let range = (self.abs_y_max - self.abs_y_min).max(1) as f32;
                        self.y = ((ev.value - self.abs_y_min) as f32 / range * self.screen_h as f32) as i32;
                        self.y = self.y.clamp(0, self.screen_h as i32 - 1);
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }

    fn poll_legacy_mouse(&mut self) -> Option<MouseEvent> {
        let mut buf = [0u8; 3];
        let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, 3) };
        if n < 3 { return None; }
        let left  = (buf[0] & 0x01) != 0;
        let dx    = buf[1] as i8 as i32;
        let dy    = -(buf[2] as i8 as i32);
        self.x = (self.x + dx).clamp(0, self.screen_w as i32 - 1);
        self.y = (self.y + dy).clamp(0, self.screen_h as i32 - 1);
        Some(MouseEvent { x: self.x, y: self.y, event_type: if left { MouseEventType::LeftDown } else { MouseEventType::Motion } })
    }
}

impl Drop for MouseInput {
    fn drop(&mut self) { unsafe { libc::close(self.fd); } }
}

unsafe impl Send for MouseInput {}

/// 抖动滤波器
pub struct JitterFilter {
    last_x: i32, last_y: i32, threshold: i32,
}

impl JitterFilter {
    pub fn new(threshold: i32) -> Self { JitterFilter { last_x: -1, last_y: -1, threshold } }
    pub fn filter(&mut self, x: i32, y: i32) -> Option<(i32, i32)> {
        if self.last_x >= 0 && (x - self.last_x).abs() < self.threshold && (y - self.last_y).abs() < self.threshold {
            return None;
        }
        self.last_x = x; self.last_y = y;
        Some((x, y))
    }
    pub fn reset(&mut self) { self.last_x = -1; self.last_y = -1; }
}