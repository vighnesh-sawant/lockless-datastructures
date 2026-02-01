use std::f64;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::atomic_ring_buffer_spsc::AtomicRingBufferSpsc;
use crate::primitives::Arc;

const CAPACITY: usize = 32;
///A wasm simulator for the ring buffer
#[wasm_bindgen]
pub struct Simulation {
    buffer: Arc<AtomicRingBufferSpsc<u32, CAPACITY>>,
    canvas: Option<CanvasRenderingContext2d>,
    width: f64,
    height: f64,
    producer_acc: f64,
    consumer_acc: f64,
    item_counter: u32,
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Simulation {
    pub fn new() -> Simulation {
        Simulation {
            buffer: AtomicRingBufferSpsc::new(),
            canvas: None,
            width: 800.0,
            height: 600.0,
            producer_acc: 0.0,
            consumer_acc: 0.0,
            item_counter: 0,
        }
    }

    pub fn attach_canvas(&mut self, canvas_id: &str) -> Result<(), JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document.get_element_by_id(canvas_id).unwrap();
        let canvas: HtmlCanvasElement = canvas.dyn_into()?;
        self.width = canvas.width() as f64;
        self.height = canvas.height() as f64;
        let ctx = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;
        self.canvas = Some(ctx);
        Ok(())
    }

    pub fn tick(&mut self, producer_speed: f64, consumer_speed: f64) {
        self.producer_acc += producer_speed;
        while self.producer_acc >= 1.0 {
            self.item_counter = self.item_counter.wrapping_add(1);
            self.buffer.push(self.item_counter).ok();
            self.producer_acc -= 1.0;
        }

        self.consumer_acc += consumer_speed;
        while self.consumer_acc >= 1.0 {
            self.buffer.pop();
            self.consumer_acc -= 1.0;
        }

        self.draw();
    }
    #[allow(deprecated)]
    fn draw(&self) {
        if let Some(ctx) = &self.canvas {
            ctx.set_fill_style(&JsValue::from_str("#1a1a1a"));
            ctx.fill_rect(0.0, 0.0, self.width, self.height);

            let center_x = self.width / 2.0;
            let center_y = self.height / 2.0;
            let radius = 150.0;
            let slot_radius = 15.0;

            for i in 0..CAPACITY {
                let angle = (i as f64 / CAPACITY as f64) * 2.0 * f64::consts::PI;
                let x = center_x + radius * angle.cos();
                let y = center_y + radius * angle.sin();

                let color = if self.buffer.exists(i) {
                    "#ff4d4d"
                } else {
                    "#4dff88"
                };

                if i == self.buffer.read_head() {
                    ctx.set_stroke_style(&JsValue::from_str("white"));
                    ctx.set_line_width(4.0);
                } else if i == self.buffer.read_tail() {
                    ctx.set_stroke_style(&JsValue::from_str("yellow"));
                    ctx.set_line_width(4.0);
                } else {
                    ctx.set_stroke_style(&JsValue::from_str("transparent"));
                }

                ctx.begin_path();
                ctx.arc(x, y, slot_radius, 0.0, 2.0 * f64::consts::PI)
                    .unwrap();
                ctx.set_fill_style(&JsValue::from_str(color));
                ctx.fill();
                ctx.stroke();
            }

            let legend_x = 20.0;
            let legend_y = 20.0;

            ctx.set_fill_style(&JsValue::from_str("white"));
            ctx.set_font("bold 16px sans-serif");
            ctx.fill_text("Buffer Status:", legend_x, legend_y).unwrap();

            ctx.begin_path();
            ctx.arc(
                legend_x + 10.0,
                legend_y + 25.0,
                6.0,
                0.0,
                2.0 * f64::consts::PI,
            )
            .unwrap();
            ctx.set_fill_style(&JsValue::from_str("#ff4d4d"));
            ctx.fill();

            ctx.set_fill_style(&JsValue::from_str("#ffcccc"));
            ctx.set_font("14px sans-serif");
            ctx.fill_text("Occupied (Data)", legend_x + 25.0, legend_y + 30.0)
                .unwrap();

            ctx.begin_path();
            ctx.arc(
                legend_x + 10.0,
                legend_y + 50.0,
                6.0,
                0.0,
                2.0 * f64::consts::PI,
            )
            .unwrap();
            ctx.set_fill_style(&JsValue::from_str("#4dff88"));
            ctx.fill();

            ctx.set_fill_style(&JsValue::from_str("#ccffcc"));
            ctx.fill_text("Free Slot (Empty)", legend_x + 25.0, legend_y + 55.0)
                .unwrap();

            ctx.set_line_width(2.0);

            ctx.set_stroke_style(&JsValue::from_str("white"));
            ctx.begin_path();
            ctx.arc(
                legend_x + 10.0,
                legend_y + 76.0,
                6.0,
                0.0,
                2.0 * f64::consts::PI,
            )
            .unwrap();
            ctx.stroke();

            ctx.set_fill_style(&JsValue::from_str("white"));
            ctx.fill_text(
                "White Ring = Head (Writer)",
                legend_x + 25.0,
                legend_y + 81.0,
            )
            .unwrap();

            ctx.set_stroke_style(&JsValue::from_str("yellow"));
            ctx.begin_path();
            ctx.arc(
                legend_x + 10.0,
                legend_y + 101.0,
                6.0,
                0.0,
                2.0 * f64::consts::PI,
            )
            .unwrap();
            ctx.stroke();

            ctx.fill_text(
                "Yellow Ring = Tail (Reader)",
                legend_x + 25.0,
                legend_y + 106.0,
            )
            .unwrap();
        }
    }
}
