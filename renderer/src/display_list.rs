use std::{collections::hash_map::HashMap, rc::Rc, vec::Vec};

use cgmath::SquareMatrix;
use glow::HasContext;
use itertools::izip;
use ldraw::{
    color::{ColorReference, Material},
    document::{Document, MultipartDocument},
    Matrix4, PartAlias, Vector4,
};
use ldraw_ir::geometry::BoundingBox3;

use crate::utils::cast_as_bytes;

pub struct DisplayItemBuilder {
    name: PartAlias,
    matrices: Vec<Matrix4>,
    colors: Vec<ColorReference>,
}

impl DisplayItemBuilder {
    pub fn new(name: PartAlias) -> Self {
        DisplayItemBuilder {
            name,
            matrices: vec![],
            colors: vec![],
        }
    }
}

pub struct InstanceBuffer<GL: HasContext> {
    gl: Rc<GL>,

    pub count: usize,

    pub model_view_matrices: Vec<Matrix4>,
    pub materials: Vec<Material>,
    pub colors: Vec<Vector4>,
    pub edge_colors: Vec<Vector4>,

    pub model_view_matrices_buffer: Option<GL::Buffer>,
    pub color_buffer: Option<GL::Buffer>,
    pub edge_color_buffer: Option<GL::Buffer>,

    modified: bool,
}

impl<GL: HasContext> InstanceBuffer<GL> {
    pub fn new(gl: Rc<GL>) -> Self {
        InstanceBuffer {
            gl,

            count: 0,

            model_view_matrices: vec![],
            materials: vec![],
            colors: vec![],
            edge_colors: vec![],

            model_view_matrices_buffer: None,
            color_buffer: None,
            edge_color_buffer: None,

            modified: false,
        }
    }

    pub fn calculate_bounding_box(&self, bounding_box: &BoundingBox3) -> Option<BoundingBox3> {
        let mut bb = BoundingBox3::zero();

        for matrix in self.model_view_matrices.iter() {
            for point in bounding_box.points() {
                let transformed = matrix * point.extend(1.0);
                bb.update_point(&transformed.truncate());
            }
        }

        if bb.is_null() {
            None
        } else {
            Some(bb)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn update_buffer(&mut self, gl: &GL) {
        if !self.modified {
            return;
        }

        if self.model_view_matrices.is_empty() {
            self.model_view_matrices_buffer = None;
        } else {
            if self.model_view_matrices_buffer.is_none() {
                self.model_view_matrices_buffer = unsafe { gl.create_buffer().ok() };
            }

            let mut buffer = Vec::<f32>::new();
            self.model_view_matrices
                .iter()
                .for_each(|e| buffer.extend(AsRef::<[f32; 16]>::as_ref(e)));

            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, self.model_view_matrices_buffer);
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    cast_as_bytes(buffer.as_ref()),
                    glow::DYNAMIC_DRAW,
                );
            }
        }

        if self.colors.is_empty() {
            self.color_buffer = None;
        } else {
            if self.color_buffer.is_none() {
                self.color_buffer = unsafe { gl.create_buffer().ok() };
            }

            let mut buffer = Vec::<f32>::new();
            self.colors
                .iter()
                .for_each(|e| buffer.extend(AsRef::<[f32; 4]>::as_ref(e)));

            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, self.color_buffer);
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    cast_as_bytes(buffer.as_ref()),
                    glow::DYNAMIC_DRAW,
                );
            }
        }

        if self.edge_colors.is_empty() {
            self.edge_color_buffer = None;
        } else {
            if self.edge_color_buffer.is_none() {
                self.edge_color_buffer = unsafe { gl.create_buffer().ok() };
            }

            let mut buffer = Vec::<f32>::new();
            self.edge_colors
                .iter()
                .for_each(|e| buffer.extend(AsRef::<[f32; 4]>::as_ref(e)));

            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, self.edge_color_buffer);
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    cast_as_bytes(buffer.as_ref()),
                    glow::DYNAMIC_DRAW,
                );
            }
        }

        self.modified = false;
    }
}

impl<GL: HasContext> Drop for InstanceBuffer<GL> {
    fn drop(&mut self) {
        let gl = &self.gl;

        unsafe {
            if let Some(b) = self.model_view_matrices_buffer {
                gl.delete_buffer(b);
            }
            if let Some(b) = self.color_buffer {
                gl.delete_buffer(b);
            }
            if let Some(b) = self.edge_color_buffer {
                gl.delete_buffer(b);
            }
        }
    }
}

pub struct DisplayItem<GL: HasContext> {
    pub part: PartAlias,

    pub opaque: InstanceBuffer<GL>,
    pub translucent: InstanceBuffer<GL>,
}

impl<GL: HasContext> DisplayItem<GL> {
    pub fn new(gl: Rc<GL>, alias: &PartAlias) -> Self {
        DisplayItem {
            part: alias.clone(),

            opaque: InstanceBuffer::new(Rc::clone(&gl)),
            translucent: InstanceBuffer::new(Rc::clone(&gl)),
        }
    }

    /* TODO: This is temporary; should be superseded with sophisticated editor stuffs */
    pub fn update_data(
        &mut self,
        opaque: bool,
        model_view_matrices: &[Matrix4],
        materials: &[Material],
    ) {
        let mut new_model_view_matrices = vec![];
        let mut new_materials = vec![];
        let mut new_colors = vec![];
        let mut new_edge_colors = vec![];
        for (model_view_matrix, material) in izip!(model_view_matrices, materials) {
            new_model_view_matrices.push(*model_view_matrix);
            new_materials.push(material.clone());
            new_colors.push(material.color.into());
            new_edge_colors.push(material.edge.into());
        }

        let buffer = if opaque {
            &mut self.opaque
        } else {
            &mut self.translucent
        };

        buffer.model_view_matrices = new_model_view_matrices;
        buffer.materials = new_materials;
        buffer.colors = new_colors;
        buffer.edge_colors = new_edge_colors;
        buffer.count = model_view_matrices.len();
        buffer.modified = true;
    }

    pub fn add(&mut self, matrix: &Matrix4, material: &Material) {
        let buffer = if material.is_translucent() {
            &mut self.translucent
        } else {
            &mut self.opaque
        };

        buffer.model_view_matrices.push(*matrix);
        buffer.materials.push(material.clone());
        buffer.colors.push(Vector4::from(&material.color));
        buffer.edge_colors.push(Vector4::from(&material.edge));
        buffer.count += 1;
        buffer.modified = true;
    }
}

pub struct DisplayList<GL: HasContext> {
    pub map: HashMap<PartAlias, DisplayItem<GL>>,
}

impl<GL: HasContext> DisplayList<GL> {
    pub fn count(&self) -> usize {
        let mut count = 0;

        for v in self.map.values() {
            count += v.opaque.count + v.translucent.count;
        }

        count
    }
}

impl<GL: HasContext> Default for DisplayList<GL> {
    fn default() -> Self {
        DisplayList {
            map: HashMap::new(),
        }
    }
}

fn build_display_list<'a, GL: HasContext>(
    gl: Rc<GL>,
    display_list: &mut DisplayList<GL>,
    document: &'a Document,
    matrix: Matrix4,
    material_stack: &mut Vec<Material>,
    parent: &'a MultipartDocument,
) {
    for e in document.iter_refs() {
        if parent.subparts.contains_key(&e.name) {
            material_stack.push(match &e.color {
                ColorReference::Material(m) => m.clone(),
                _ => material_stack.last().unwrap().clone(),
            });

            build_display_list(
                Rc::clone(&gl),
                display_list,
                parent.subparts.get(&e.name).unwrap(),
                matrix * e.matrix,
                material_stack,
                parent,
            );

            material_stack.pop();
        } else {
            let material = match &e.color {
                ColorReference::Material(m) => m,
                _ => material_stack.last().unwrap(),
            };

            display_list.add(
                Rc::clone(&gl),
                e.name.clone(),
                matrix * e.matrix,
                material.clone(),
            );
        }
    }
}

impl<GL: HasContext> DisplayList<GL> {
    pub fn from_multipart_document(gl: Rc<GL>, document: &MultipartDocument) -> Self {
        let mut display_list = DisplayList::default();
        let mut material_stack = vec![Material::default()];

        build_display_list(
            gl,
            &mut display_list,
            &document.body,
            Matrix4::identity(),
            &mut material_stack,
            document,
        );

        display_list
    }

    pub fn add(&mut self, gl: Rc<GL>, name: PartAlias, matrix: Matrix4, material: Material) {
        self.map
            .entry(name.clone())
            .or_insert_with(|| DisplayItem::new(Rc::clone(&gl), &name))
            .add(&matrix, &material);
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}
