//! 엔티티마다 한 칸씩 쓰는 dynamic-offset uniform 버퍼.
//!
//! 한 프레임의 모든 엔티티 uniform 을 버퍼 하나에 `stride` 간격으로 올리고, draw
//! 마다 `set_bind_group(.., &[buffer.offset(i)])` 로 자기 칸을 고른다.

use std::marker::PhantomData;

use voltex_renderer::align_up;

/// `capacity` 개의 `T` 를 `stride` 바이트 간격으로 담는 uniform 버퍼.
pub(crate) struct DynamicUniformBuffer<T> {
    pub(crate) buffer: wgpu::Buffer,
    /// 칸 사이 간격 — `size_of::<T>()` 를 GPU 의
    /// `min_uniform_buffer_offset_alignment` 배수로 올린 값.
    pub(crate) stride: u32,
    _entry: PhantomData<T>,
}

impl<T: bytemuck::Pod> DynamicUniformBuffer<T> {
    pub(crate) fn new(device: &wgpu::Device, label: &str, alignment: u32, capacity: usize) -> Self {
        let stride = align_up(std::mem::size_of::<T>() as u32, alignment);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (stride as usize * capacity) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            stride,
            _entry: PhantomData,
        }
    }

    /// 한 칸(`size_of::<T>()` 바이트)만 보이는 바인딩. dynamic offset 이 칸을 옮긴다.
    pub(crate) fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: wgpu::BufferSize::new(std::mem::size_of::<T>() as u64),
        })
    }

    /// `index` 번째 칸의 dynamic offset.
    pub(crate) fn offset(&self, index: usize) -> u32 {
        index as u32 * self.stride
    }

    /// `entries[i]` 를 `i` 번째 칸에 올린다. 칸 사이 패딩은 0. 빈 슬라이스면 아무것도 안 한다.
    pub(crate) fn upload(&self, queue: &wgpu::Queue, entries: &[T]) {
        if entries.is_empty() {
            return;
        }
        let stride = self.stride as usize;
        let mut staging = vec![0u8; entries.len() * stride];
        for (slot, entry) in staging.chunks_exact_mut(stride).zip(entries) {
            let bytes = bytemuck::bytes_of(entry);
            slot[..bytes.len()].copy_from_slice(bytes);
        }
        queue.write_buffer(&self.buffer, 0, &staging);
    }
}
