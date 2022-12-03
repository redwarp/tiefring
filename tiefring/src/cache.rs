use std::{collections::BTreeMap, mem::size_of};

use wgpu::{util::DeviceExt, Buffer, BufferSlice, BufferUsages, Device, Queue};

#[derive(Debug)]
pub(crate) struct BufferCache {
    vertex_map: BTreeMap<u64, Vec<ReusableBuffer>>,
    index_map: BTreeMap<u64, Vec<ReusableBuffer>>,
}

impl BufferCache {
    pub fn new() -> Self {
        let vertex_map = BTreeMap::new();
        let index_map = BTreeMap::new();
        Self {
            vertex_map,
            index_map,
        }
    }

    pub fn get_buffer(
        &mut self,
        device: &Device,
        queue: &Queue,
        content: &[u8],
        usage: BufferUsages,
    ) -> ReusableBuffer {
        let capacity = (size_of::<u8>() * content.len()) as u64;

        let buffer = self.buffer_with_capacity(capacity, usage);

        if let Some(mut buffer) = buffer {
            buffer.update(queue, content);
            buffer
        } else {
            ReusableBuffer::new(device, content, usage | BufferUsages::COPY_DST)
        }
    }

    pub fn release_buffer(&mut self, buffer: ReusableBuffer) {
        if (buffer.usage & BufferUsages::COPY_DST).is_empty() {
            return;
        }

        let map = match buffer.usage {
            usage if usage & BufferUsages::VERTEX == BufferUsages::VERTEX => &mut self.vertex_map,
            usage if usage & BufferUsages::INDEX == BufferUsages::INDEX => &mut self.index_map,
            _ => return,
        };

        map.entry(buffer.max_size)
            .or_insert_with(Vec::new)
            .push(buffer);
    }

    pub fn clear(&mut self) {
        self.vertex_map.clear();
        self.index_map.clear();
    }

    fn buffer_with_capacity(
        &mut self,
        capacity: u64,
        usage: BufferUsages,
    ) -> Option<ReusableBuffer> {
        let map = match usage {
            usage if usage & BufferUsages::VERTEX == BufferUsages::VERTEX => &mut self.vertex_map,
            usage if usage & BufferUsages::INDEX == BufferUsages::INDEX => &mut self.index_map,
            _ => return None,
        };

        let key = map.range(capacity..).next().map(|(key, _)| key).copied();

        let buffer = if let Some(key) = key {
            let buffers = map.get_mut(&key).expect("We searched for the key");
            let buffer = buffers.pop();
            if buffers.is_empty() {
                map.remove(&key);
            }
            buffer
        } else {
            None
        };

        buffer
    }
}

#[derive(Debug)]
pub(crate) struct ReusableBuffer {
    pub buffer: Buffer,
    pub usage: BufferUsages,
    pub max_size: u64,
    pub current_size: u64,
}

impl ReusableBuffer {
    pub fn new(device: &Device, content: &[u8], usage: BufferUsages) -> Self {
        let current_size = (size_of::<u8>() * content.len()) as u64;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: content,
            usage,
        });

        Self {
            buffer,
            usage,
            max_size: current_size,
            current_size,
        }
    }

    pub fn slice(&self) -> BufferSlice {
        self.buffer.slice(..self.current_size)
    }

    pub fn update(&mut self, queue: &Queue, content: &[u8]) {
        let current_size = (size_of::<u8>() * content.len()) as u64;
        queue.write_buffer(&self.buffer, 0, content);

        self.current_size = current_size;
    }
}
