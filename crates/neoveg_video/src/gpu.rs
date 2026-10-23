//! GPU upload of CPU frames (wgpu).

use crate::frame::FrameBuffer;

/// Creates an `Rgba8UnormSrgb` texture holding `frame`.
pub fn upload_frame(device: &wgpu::Device, queue: &wgpu::Queue, frame: &FrameBuffer) -> wgpu::Texture {
    let size = wgpu::Extent3d { width: frame.width.max(1), height: frame.height.max(1), depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("neoveg_frame"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &frame.rgba,
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * size.width), rows_per_image: Some(size.height) },
        size,
    );
    texture
}
