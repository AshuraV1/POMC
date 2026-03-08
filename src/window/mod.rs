pub mod input;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use thiserror::Error;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::net::NetworkEvent;
use crate::renderer::Renderer;
use crate::world::chunk::ChunkStore;
use input::InputState;

#[derive(Error, Debug)]
pub enum WindowError {
    #[error("failed to create event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    #[error("failed to create window: {0}")]
    CreateWindow(#[from] winit::error::OsError),

    #[error("renderer error: {0}")]
    Renderer(#[from] crate::renderer::RendererError),
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    input: InputState,
    last_frame: Option<Instant>,
    net_events: Option<crossbeam_channel::Receiver<NetworkEvent>>,
    chunk_store: ChunkStore,
    assets_dir: PathBuf,
    position_set: bool,
}

impl App {
    fn new(
        net_events: Option<crossbeam_channel::Receiver<NetworkEvent>>,
        assets_dir: PathBuf,
    ) -> Self {
        Self {
            window: None,
            renderer: None,
            input: InputState::new(),
            last_frame: None,
            net_events,
            chunk_store: ChunkStore::new(8),
            assets_dir,
            position_set: false,
        }
    }

    fn apply_cursor_grab(&self) {
        let Some(window) = &self.window else { return };
        if self.input.is_cursor_captured() {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            window.set_cursor_visible(false);
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }

    fn drain_network_events(&mut self) {
        let Some(rx) = &self.net_events else { return };
        let mut chunks_to_mesh = Vec::new();

        while let Ok(event) = rx.try_recv() {
            match event {
                NetworkEvent::Connected => {
                    log::info!("Connected to server");
                }
                NetworkEvent::ChunkLoaded { pos, data, heightmaps } => {
                    if let Err(e) = self.chunk_store.load_chunk(pos, &data, &heightmaps) {
                        log::error!("Failed to load chunk [{}, {}]: {e}", pos.x, pos.z);
                        continue;
                    }
                    chunks_to_mesh.push(pos);
                }
                NetworkEvent::ChunkUnloaded { pos } => {
                    self.chunk_store.unload_chunk(&pos);
                    if let Some(renderer) = &mut self.renderer {
                        renderer.remove_chunk_mesh(&pos);
                    }
                }
                NetworkEvent::ChunkCacheCenter { x, z } => {
                    self.chunk_store.set_center(azalea_core::position::ChunkPos::new(x, z));
                }
                NetworkEvent::PlayerPosition { x, y, z, yaw, pitch, .. } => {
                    if !self.position_set {
                        if let Some(renderer) = &mut self.renderer {
                            renderer.set_camera_position(x, y, z, yaw, pitch);
                        }
                        self.position_set = true;
                        log::info!("Player position set to ({x:.1}, {y:.1}, {z:.1})");
                    }
                }
                NetworkEvent::Disconnected { reason } => {
                    log::warn!("Disconnected: {reason}");
                }
            }
        }

        if let Some(renderer) = &mut self.renderer {
            for pos in chunks_to_mesh {
                let mesh = renderer.mesh_chunk(&self.chunk_store, pos);
                renderer.upload_chunk_mesh(&mesh);
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Ferrite")
            .with_inner_size(winit::dpi::LogicalSize::new(854, 480));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let renderer = match Renderer::new(Arc::clone(&window), &self.assets_dir) {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to create renderer: {e}");
                event_loop.exit();
                return;
            }
        };

        self.renderer = Some(renderer);
        self.window = Some(window);
        self.apply_cursor_grab();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(new_size);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        self.input.toggle_cursor_capture();
                        self.apply_cursor_grab();
                    }
                }
                self.input.on_key_event(&event);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = self
                    .last_frame
                    .map(|last| now.duration_since(last).as_secs_f32())
                    .unwrap_or(0.0);
                self.last_frame = Some(now);

                self.drain_network_events();

                if let Some(renderer) = &mut self.renderer {
                    renderer.update(&mut self.input, dt);
                    if let Err(e) = renderer.render() {
                        log::error!("Render error: {e}");
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.input.is_cursor_captured() {
                self.input.on_mouse_motion(delta);
            }
        }
    }
}

pub fn run(
    net_events: Option<crossbeam_channel::Receiver<NetworkEvent>>,
    assets_dir: PathBuf,
) -> Result<(), WindowError> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new(net_events, assets_dir);
    event_loop.run_app(&mut app)?;
    Ok(())
}
