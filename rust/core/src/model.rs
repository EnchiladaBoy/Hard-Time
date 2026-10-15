//! Bounded 3DS static-geometry reader. Positions remain in the file's Z-up space.
//! Mesh local matrices are retained for the later animation port. 3DS stores
//! static vertices in world space, so applying that matrix again is incorrect.
use crate::legacy::{DecodeError, Reader};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, DecodeError>;
const MAX_FILE: usize = 64 * 1024 * 1024;
const MAX_OBJECTS: usize = 10_000;

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub diffuse: [f32; 3],
    pub opacity: f32,
    pub texture: Option<String>,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct Object {
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub faces: Vec<[u16; 3]>,
    pub materials: BTreeMap<String, Vec<u16>>,
    pub local_matrix: Option<[f32; 12]>,
}

#[derive(Debug, Default)]
pub struct Model {
    pub materials: BTreeMap<String, Material>,
    pub objects: Vec<Object>,
    pub has_animation: bool,
}

struct Chunk<'a> {
    id: u16,
    bytes: &'a [u8],
}

fn chunks(mut bytes: &[u8]) -> Result<Vec<Chunk<'_>>> {
    let mut result = Vec::new();
    while !bytes.is_empty() {
        let mut reader = Reader::new(bytes);
        let id = reader.u16()?;
        let length = reader.u32()? as usize;
        if length < 6 {
            return Err(reader.error("3DS chunk length is less than its header"));
        }
        let body = reader.take(length - 6)?;
        result.push(Chunk { id, bytes: body });
        bytes = &bytes[length..];
        if result.len() > 100_000 {
            return Err(reader.error("Too many 3DS chunks"));
        }
    }
    Ok(result)
}

fn c_string(reader: &mut Reader<'_>) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let byte = reader.take(1)?[0];
        if byte == 0 {
            break;
        }
        if bytes.len() >= 1024 {
            return Err(reader.error("3DS name exceeds limit"));
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| reader.error("Unsupported non-UTF-8 3DS name"))
}

impl Model {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE {
            return Err(DecodeError {
                offset: 0,
                message: "3DS file exceeds size limit",
            });
        }
        let root = chunks(bytes)?;
        if root.len() != 1 || root[0].id != 0x4d4d {
            return Err(DecodeError {
                offset: 0,
                message: "Expected a single 3DS root chunk",
            });
        }
        let mut model = Self::default();
        for chunk in chunks(root[0].bytes)? {
            match chunk.id {
                0x3d3d => model.scene(chunk.bytes)?,
                0xb000 => model.has_animation = true,
                _ => {}
            }
        }
        for object in &model.objects {
            for face in &object.faces {
                if face
                    .iter()
                    .any(|&index| usize::from(index) >= object.vertices.len())
                {
                    return Err(DecodeError {
                        offset: 0,
                        message: "3DS triangle references a missing vertex",
                    });
                }
            }
            if !object.uv.is_empty() && object.uv.len() != object.vertices.len() {
                return Err(DecodeError {
                    offset: 0,
                    message: "3DS texture coordinates do not match vertices",
                });
            }
            for faces in object.materials.values() {
                for &index in faces {
                    if usize::from(index) >= object.faces.len() {
                        return Err(DecodeError {
                            offset: 0,
                            message: "3DS material references a missing triangle",
                        });
                    }
                }
            }
        }
        Ok(model)
    }

    fn scene(&mut self, bytes: &[u8]) -> Result<()> {
        for chunk in chunks(bytes)? {
            match chunk.id {
                0xafff => {
                    let material = material(chunk.bytes)?;
                    if self
                        .materials
                        .insert(material.name.clone(), material)
                        .is_some()
                    {
                        return Err(DecodeError {
                            offset: 0,
                            message: "Duplicate 3DS material",
                        });
                    }
                }
                0x4000 => {
                    if self.objects.len() >= MAX_OBJECTS {
                        return Err(DecodeError {
                            offset: 0,
                            message: "Too many 3DS objects",
                        });
                    }
                    let mut reader = Reader::new(chunk.bytes);
                    let name = c_string(&mut reader)?;
                    for child in chunks(&chunk.bytes[reader.position..])? {
                        if child.id == 0x4100 {
                            if self.objects.len() >= MAX_OBJECTS {
                                return Err(reader.error("Too many 3DS objects"));
                            }
                            self.objects.push(object(name.clone(), child.bytes)?);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn triangle_count(&self) -> usize {
        self.objects.iter().map(|obj| obj.faces.len()).sum()
    }
    pub fn vertex_count(&self) -> usize {
        self.objects.iter().map(|obj| obj.vertices.len()).sum()
    }
}

fn object(name: String, bytes: &[u8]) -> Result<Object> {
    let mut object = Object {
        name,
        ..Default::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    for chunk in chunks(bytes)? {
        let mut reader = Reader::new(chunk.bytes);
        if matches!(chunk.id, 0x4110 | 0x4120 | 0x4140 | 0x4160) && !seen.insert(chunk.id) {
            return Err(reader.error("Duplicate 3DS mesh data chunk"));
        }
        match chunk.id {
            0x4110 => {
                let count = reader.u16()?;
                for _ in 0..count {
                    object
                        .vertices
                        .push([reader.f32()?, reader.f32()?, reader.f32()?]);
                }
                reader.finished()?;
            }
            0x4140 => {
                let count = reader.u16()?;
                for _ in 0..count {
                    object.uv.push([reader.f32()?, reader.f32()?]);
                }
                reader.finished()?;
            }
            0x4120 => {
                let count = reader.u16()?;
                let mut face_materials = vec![None; usize::from(count)];
                for _ in 0..count {
                    object
                        .faces
                        .push([reader.u16()?, reader.u16()?, reader.u16()?]);
                    reader.u16()?; // Original face edge visibility flags.
                }
                for group in chunks(&chunk.bytes[reader.position..])? {
                    if group.id == 0x4130 {
                        let mut r = Reader::new(group.bytes);
                        let name = c_string(&mut r)?;
                        let count = r.u16()?;
                        for _ in 0..count {
                            let face = r.u16()?;
                            let Some(material) = face_materials.get_mut(usize::from(face)) else {
                                return Err(r.error("3DS material references a missing triangle"));
                            };
                            // Original assets reassign faces. The reference loader's
                            // parseFaceMat uses the last assignment in file order.
                            *material = Some(name.clone());
                        }
                        r.finished()?;
                    }
                }
                for (face, name) in face_materials.into_iter().enumerate() {
                    if let Some(name) = name {
                        object.materials.entry(name).or_default().push(face as u16);
                    }
                }
            }
            0x4160 => {
                let mut matrix = [0.0; 12];
                for v in &mut matrix {
                    *v = reader.f32()?;
                }
                reader.finished()?;
                object.local_matrix = Some(matrix);
            }
            _ => {}
        }
    }
    Ok(object)
}

fn material(bytes: &[u8]) -> Result<Material> {
    let mut result = Material {
        name: String::new(),
        diffuse: [1.0; 3],
        opacity: 1.0,
        texture: None,
        uv_scale: [1.0; 2],
        uv_offset: [0.0; 2],
    };
    for chunk in chunks(bytes)? {
        let mut reader = Reader::new(chunk.bytes);
        match chunk.id {
            0xa000 => result.name = c_string(&mut reader)?,
            0xa020 => {
                for color in chunks(chunk.bytes)? {
                    let mut r = Reader::new(color.bytes);
                    match color.id {
                        0x0010 | 0x0013 => result.diffuse = [r.f32()?, r.f32()?, r.f32()?],
                        0x0011 | 0x0012 => {
                            let rgb = r.take(3)?;
                            result.diffuse = [
                                rgb[0] as f32 / 255.0,
                                rgb[1] as f32 / 255.0,
                                rgb[2] as f32 / 255.0,
                            ];
                        }
                        _ => {}
                    }
                }
            }
            0xa050 => {
                for percent in chunks(chunk.bytes)? {
                    let mut r = Reader::new(percent.bytes);
                    match percent.id {
                        0x0030 => result.opacity = 1.0 - r.u16()? as f32 / 100.0,
                        0x0031 => result.opacity = 1.0 - r.f32()?,
                        _ => {}
                    }
                }
                result.opacity = result.opacity.clamp(0.0, 1.0);
            }
            0xa200 => {
                for map in chunks(chunk.bytes)? {
                    let mut r = Reader::new(map.bytes);
                    match map.id {
                        0xa300 => result.texture = Some(c_string(&mut r)?),
                        0xa354 => result.uv_scale[0] = r.f32()?,
                        0xa356 => result.uv_scale[1] = r.f32()?,
                        0xa358 => result.uv_offset[0] = r.f32()?,
                        0xa35a => result.uv_offset[1] = r.f32()?,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(id: u16, body: Vec<u8>) -> Vec<u8> {
        let mut bytes = id.to_le_bytes().to_vec();
        bytes.extend(((body.len() + 6) as u32).to_le_bytes());
        bytes.extend(body);
        bytes
    }
    fn triangle(index: u16) -> Vec<u8> {
        triangle_with_materials(index, &[])
    }
    fn triangle_with_materials(index: u16, materials: &[(&str, u16)]) -> Vec<u8> {
        let mut vertices = 3_u16.to_le_bytes().to_vec();
        for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            vertices.extend(value.to_le_bytes());
        }
        let mut faces = 1_u16.to_le_bytes().to_vec();
        for value in [0, 1, index, 0] {
            faces.extend(value.to_le_bytes());
        }
        for &(name, face) in materials {
            let mut group = name.as_bytes().to_vec();
            group.push(0);
            group.extend(1_u16.to_le_bytes());
            group.extend(face.to_le_bytes());
            faces.extend(chunk(0x4130, group));
        }
        let mut mesh = chunk(0x4110, vertices);
        mesh.extend(chunk(0x4120, faces));
        let mut named = b"Floor\0".to_vec();
        named.extend(chunk(0x4100, mesh));
        chunk(0x4d4d, chunk(0x3d3d, chunk(0x4000, named)))
    }
    #[test]
    fn loads_geometry_and_rejects_invalid_indices() {
        let bytes = triangle(2);
        let model = Model::decode(&bytes).unwrap();
        assert_eq!(model.triangle_count(), 1);
        assert_eq!(model.objects[0].name, "Floor");
        assert_eq!(model.objects[0].vertices[1], [1.0, 0.0, 0.0]);
        assert!(Model::decode(&triangle(3)).is_err());
        for length in 0..bytes.len() {
            assert!(Model::decode(&bytes[..length]).is_err());
        }
    }
    #[test]
    fn later_material_assignments_replace_earlier_ones() {
        let bytes = triangle_with_materials(2, &[("First", 0), ("Last", 0)]);
        let model = Model::decode(&bytes).unwrap();
        assert_eq!(model.objects[0].materials.len(), 1);
        assert_eq!(model.objects[0].materials["Last"], vec![0]);
        assert!(Model::decode(&triangle_with_materials(2, &[("Outside", 1)])).is_err());
    }
    #[test]
    fn duplicate_geometry_chunks_are_rejected() {
        let vertices = chunk(0x4110, 0_u16.to_le_bytes().to_vec());
        let mut repeated = vertices.clone();
        repeated.extend(vertices);
        assert!(object("Bad mesh".into(), &repeated).is_err());
    }
    #[test]
    fn chunk_lengths_cannot_escape_their_parent_or_stall_the_parser() {
        for length in [0, 1, 5, u32::MAX] {
            let mut bytes = vec![0x4d, 0x4d];
            bytes.extend(length.to_le_bytes());
            assert!(Model::decode(&bytes).is_err());
        }
        let mut bytes = triangle(2);
        bytes.push(0);
        assert!(Model::decode(&bytes).is_err());
    }
}
