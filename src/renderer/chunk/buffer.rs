use wgpu::util::DeviceExt;

use super::mesher::ChunkMeshData;

pub struct ChunkMeshBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl ChunkMeshBuffers {
    pub fn from_mesh_data(device: &wgpu::Device, mesh: &ChunkMeshData) -> Option<Self> {
        if mesh.indices.is_empty() {
            return None;
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk_vertex_buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk_index_buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        })
    }
}
