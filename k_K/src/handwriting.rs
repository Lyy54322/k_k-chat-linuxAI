//! 手写画板主循环
//!
//! 4 条独立异步线程处理：
//! 1. 鼠标轨迹采集（evdev）
//! 2. fbdev 画布渲染
//! 3. 图像预处理（裁剪/降噪/标准化）
//! 4. 离线手写识别

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::evdev_input::{MouseInput, MouseEventType, JitterFilter};
use crate::fbdev::Framebuffer;
use crate::hwr_engine::HwrEngine;

// ── 笔画数据 ─────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Stroke {
    pub points: Vec<(i32, i32)>,
}

impl Stroke {
    pub fn new() -> Self { Stroke { points: Vec::new() } }
    pub fn add_point(&mut self, x: i32, y: i32) {
        if let Some(&(lx, ly)) = self.points.last() {
            if lx == x && ly == y { return; }
        }
        self.points.push((x, y));
    }
    pub fn is_empty(&self) -> bool { self.points.is_empty() }
}

// ── 共享状态 ──────────────────────────────────────────────────────
pub struct HandwritingState {
    pub strokes: Vec<Stroke>,
    pub pending_candidates: Vec<String>,
    pub needs_redraw: bool,
    pub has_new_stroke: bool,
    current_stroke: Option<Stroke>,
    last_pos: Option<(i32, i32)>,
    pub left_down: bool,
    pub canvas_top: u32,
}

impl HandwritingState {
    pub fn new() -> Result<Self, String> {
        Ok(HandwritingState {
            strokes: Vec::new(),
            pending_candidates: Vec::new(),
            needs_redraw: false,
            has_new_stroke: false,
            current_stroke: None,
            last_pos: None,
            left_down: false,
            canvas_top: 0,
        })
    }
    pub fn set_canvas_top(&mut self, top: u32) { self.canvas_top = top; }
}

// ── 线程主循环 ────────────────────────────────────────────────────
pub fn handwriting_loop(
    hw_state: Option<Arc<Mutex<HandwritingState>>>,
    hwr_engine: Option<Arc<Mutex<HwrEngine>>>,
    running: Arc<AtomicBool>,
) {
    let state = match hw_state {
        Some(s) => s,
        None => return,
    };

    let fb = match Framebuffer::open() {
        Ok(f) => f,
        Err(_) => { eprintln!("[手写线程] 无法打开帧缓冲"); return; }
    };

    {
        let mut s = state.lock().unwrap();
        s.set_canvas_top(fb.canvas_top);
        fb.clear_canvas();
        fb.draw_separator();
    }

    let mut mouse = match MouseInput::open(fb.width, fb.height) {
        Ok(m) => m,
        Err(e) => { eprintln!("[手写线程] 无法打开鼠标: {}", e); return; }
    };

    let mut jitter = JitterFilter::new(1);
    let mut right_click_pending = false;

    while running.load(Ordering::SeqCst) {
        if let Some(event) = mouse.poll_event() {
            let canvas_top = fb.canvas_top as i32;
            let in_canvas = event.y >= canvas_top;

            match event.event_type {
                MouseEventType::LeftDown if in_canvas => {
                    let mut s = state.lock().unwrap();
                    s.left_down = true;
                    s.current_stroke = Some(Stroke::new());
                    let local_y = event.y - canvas_top;
                    if let Some(ref mut stroke) = s.current_stroke {
                        stroke.add_point(event.x, local_y);
                    }
                    s.last_pos = Some((event.x, local_y));
                    jitter.reset();
                    right_click_pending = false;
                }
                MouseEventType::LeftUp if in_canvas => {
                    let mut s = state.lock().unwrap();
                    s.left_down = false;
                    if let Some(stroke) = s.current_stroke.take() {
                        if !stroke.is_empty() {
                            s.strokes.push(stroke);
                            s.has_new_stroke = true;
                        }
                    }
                    s.last_pos = None;
                }
                MouseEventType::RightDown if in_canvas => {
                    right_click_pending = true;
                }
                MouseEventType::Motion => {
                    // 右键 → 撤销
                    if right_click_pending {
                        let mut s = state.lock().unwrap();
                        if !s.strokes.is_empty() {
                            s.strokes.pop();
                            fb.clear_canvas();
                            for stroke in &s.strokes {
                                for i in 1..stroke.points.len() {
                                    let (x0, y0) = stroke.points[i - 1];
                                    let (x1, y1) = stroke.points[i];
                                    fb.draw_aa_line(x0, y0 + canvas_top, x1, y1 + canvas_top);
                                }
                            }
                            fb.draw_separator();
                        }
                        right_click_pending = false;
                    }

                    // 左键拖动绘图
                    if in_canvas {
                        let mut s = state.lock().unwrap();
                        if s.left_down {
                            let prev_pos = s.last_pos;
                            if let Some(ref mut stroke) = s.current_stroke {
                                let local_y = event.y - canvas_top;
                                if let Some(filtered) = jitter.filter(event.x, local_y) {
                                    if let Some((lx, ly)) = prev_pos {
                                        fb.draw_aa_line(lx, ly + canvas_top, filtered.0, filtered.1 + canvas_top);
                                    }
                                    stroke.add_point(filtered.0, filtered.1);
                                    s.last_pos = Some(filtered);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            // 无事件 → 检查是否需要手写识别
            {
                let mut s = state.lock().unwrap();
                if s.has_new_stroke && hwr_engine.is_some() {
                    s.has_new_stroke = false;
                    let pts: Vec<(i32, i32)> = s.strokes.iter().flat_map(|st| st.points.iter().cloned()).collect();
                    drop(s);
                    if pts.len() >= 3 {
                        if let Some(ref eng) = hwr_engine {
                            let cands = eng.lock().unwrap().recognize(&pts, fb.width, fb.canvas_height);
                            if !cands.is_empty() {
                                let mut s = state.lock().unwrap();
                                s.pending_candidates = cands;
                                s.strokes.clear();
                                fb.clear_canvas();
                                fb.draw_separator();
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}