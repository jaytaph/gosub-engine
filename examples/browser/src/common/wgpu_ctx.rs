use gosub_render_pipeline::render::backends::vello::WgpuContextProvider;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// [`WgpuContextProvider`] backed by a simple id → texture map.
///
/// Shared by the winit and egui vello shells; only construction differs
/// (winit creates its own device/queue, egui borrows eframe's).
pub struct TextureMapContextProvider {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    textures: RwLock<HashMap<u64, (wgpu::Texture, wgpu::TextureView)>>,
    next_id: AtomicU64,
}

impl TextureMapContextProvider {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            textures: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }
}

impl WgpuContextProvider for TextureMapContextProvider {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn device_arc(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    fn queue_arc(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    fn create_texture(&self, width: u32, height: u32, format: wgpu::TextureFormat) -> u64 {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gosub-vello-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.textures.write().insert(id, (texture, view));
        id
    }

    fn get_texture(&self, id: u64) -> Option<(wgpu::Texture, wgpu::TextureView)> {
        self.textures.read().get(&id).map(|(t, v)| (t.clone(), v.clone()))
    }

    fn remove_texture(&self, id: u64) {
        self.textures.write().remove(&id);
    }
}
