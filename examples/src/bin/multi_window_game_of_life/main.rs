// Copyright (c) 2022 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

// A multi windowed game of life application. You could use this to learn:
//
// - how to handle multiple window inputs,
// - how to draw on a canvas,
// - how to organize compute shader with graphics,
// - how to do a cellular automata simulation using compute shaders.
//
// The possibilities are limitless. ;)

mod app;
mod game_of_life;
mod pixels_draw;
mod render_pass;

use crate::app::{App, RenderPipeline};
use cgmath::Vector2;
use std::time::Instant;
use vulkano_util::renderer::VulkanoWindowRenderer;
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

pub const WINDOW_WIDTH: f32 = 1024.0;
pub const WINDOW_HEIGHT: f32 = 1024.0;
pub const WINDOW2_WIDTH: f32 = 512.0;
pub const WINDOW2_HEIGHT: f32 = 512.0;
pub const SCALING: f32 = 2.0;

fn main() {
    println!("Welcome to Vulkano Game of Life\nUse the mouse to draw life on the grid(s)\n");

    // Create event loop.
    let event_loop = EventLoop::new();

    // Create app with vulkano context.
    let mut app = App::default();
    app.open(&event_loop);

    // Time & inputs...
    let mut time = Instant::now();
    let mut cursor_pos = Vector2::new(0.0, 0.0);

    // An extremely crude way to handle input state... but works for this example.
    let mut mouse_is_pressed_w1 = false;
    let mut mouse_is_pressed_w2 = false;

    event_loop.run(move |event, _, control_flow| {
        if process_event(
            &event,
            &mut app,
            &mut cursor_pos,
            &mut mouse_is_pressed_w1,
            &mut mouse_is_pressed_w2,
        ) {
            *control_flow = ControlFlow::Exit;
            return;
        }

        // Draw life on windows if mouse is down.
        draw_life(
            &mut app,
            cursor_pos,
            mouse_is_pressed_w1,
            mouse_is_pressed_w2,
        );

        // Compute life & render 60fps.
        if (Instant::now() - time).as_secs_f64() > 1.0 / 60.0 {
            compute_then_render_per_window(&mut app);
            time = Instant::now();
        }
    })
}

/// Processes a single event for an event loop.
/// Returns true only if the window is to be closed.
pub fn process_event(
    event: &Event<()>,
    app: &mut App,
    cursor_pos: &mut Vector2<f32>,
    mouse_pressed_w1: &mut bool,
    mouse_pressed_w2: &mut bool,
) -> bool {
    if let Event::WindowEvent {
        event, window_id, ..
    } = &event
    {
        match event {
            WindowEvent::CloseRequested => {
                if *window_id == app.windows.primary_window_id().unwrap() {
                    return true;
                } else {
                    // Destroy window by removing its renderer.
                    app.windows.remove_renderer(*window_id);
                    app.pipelines.remove(window_id);
                }
            }
            // Resize window and its images.
            WindowEvent::Resized(..) | WindowEvent::ScaleFactorChanged { .. } => {
                let vulkano_window = app.windows.get_renderer_mut(*window_id).unwrap();
                vulkano_window.resize();
            }
            // Handle mouse position events.
            WindowEvent::CursorMoved { position, .. } => {
                *cursor_pos = Vector2::new(position.x as f32, position.y as f32)
            }
            // Handle mouse button events.
            WindowEvent::MouseInput { state, button, .. } => {
                let mut mouse_pressed = false;
                if button == &MouseButton::Left && state == &ElementState::Pressed {
                    mouse_pressed = true;
                }
                if button == &MouseButton::Left && state == &ElementState::Released {
                    mouse_pressed = false;
                }
                if window_id == &app.windows.primary_window_id().unwrap() {
                    *mouse_pressed_w1 = mouse_pressed;
                } else {
                    *mouse_pressed_w2 = mouse_pressed;
                }
            }
            _ => (),
        }
    }
    false
}

fn draw_life(
    app: &mut App,
    cursor_pos: Vector2<f32>,
    mouse_is_pressed_w1: bool,
    mouse_is_pressed_w2: bool,
) {
    let primary_window_id = app.windows.primary_window_id().unwrap();
    for (id, window) in app.windows.iter_mut() {
        if id == &primary_window_id && !mouse_is_pressed_w1 {
            continue;
        }
        if id != &primary_window_id && !mouse_is_pressed_w2 {
            continue;
        }

        let window_size = window.window_size();
        let compute_pipeline = &mut app.pipelines.get_mut(id).unwrap().compute;
        let mut normalized_pos = Vector2::new(
            (cursor_pos.x / window_size[0]).clamp(0.0, 1.0),
            (cursor_pos.y / window_size[1]).clamp(0.0, 1.0),
        );

        // Flip y.
        normalized_pos.y = 1.0 - normalized_pos.y;
        let image_extent = compute_pipeline.color_image().image().extent();
        compute_pipeline.draw_life(Vector2::new(
            (image_extent[0] as f32 * normalized_pos.x) as i32,
            (image_extent[1] as f32 * normalized_pos.y) as i32,
        ))
    }
}

/// Compute and render per window.
fn compute_then_render_per_window(app: &mut App) {
    let primary_window_id = app.windows.primary_window_id().unwrap();
    for (window_id, window_renderer) in app.windows.iter_mut() {
        let pipeline = app.pipelines.get_mut(window_id).unwrap();
        if *window_id == primary_window_id {
            compute_then_render(window_renderer, pipeline, [1.0, 0.0, 0.0, 1.0], [0.0; 4]);
        } else {
            compute_then_render(window_renderer, pipeline, [0.0, 0.0, 0.0, 1.0], [1.0; 4]);
        }
    }
}

/// Compute game of life, then display result on target image.
fn compute_then_render(
    window_renderer: &mut VulkanoWindowRenderer,
    pipeline: &mut RenderPipeline,
    life_color: [f32; 4],
    dead_color: [f32; 4],
) {
    // Skip this window when minimized.
    match window_renderer.window_size() {
        [w, h] => {
            if w == 0.0 || h == 0.0 {
                return;
            }
        }
    }

    // Start the frame.
    let before_pipeline_future = match window_renderer.acquire() {
        Err(e) => {
            println!("{e}");
            return;
        }
        Ok(future) => future,
    };

    // Compute.
    let after_compute = pipeline
        .compute
        .compute(before_pipeline_future, life_color, dead_color);

    // Render.
    let color_image = pipeline.compute.color_image();
    let target_image = window_renderer.swapchain_image_view();

    let after_render = pipeline
        .place_over_frame
        .render(after_compute, color_image, target_image);

    // Finish the frame. Wait for the future so resources are not in use when we render.
    window_renderer.present(after_render, true);
}
