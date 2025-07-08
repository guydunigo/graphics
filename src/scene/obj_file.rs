pub const SUZANNE_OBJ_PATH: &str = "resources/Suzanne.obj";

use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use glam::{Vec3, Vec4, vec3, vec4};
use obj::raw::{
    material::MtlColor,
    object::{Group, Polygon},
    parse_mtl, parse_obj,
};

use crate::maths::Vec3f;

use super::{mesh::Mesh, triangle::Texture, triangle::Triangle};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    // pub uv_x: f32,
    // pub normal: Vec3,
    // pub uv_y: f32,
    // pub color: Vec4,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Default::default(),
            // uv_x: Default::default(),
            // normal: vec3(1., 0., 0.),
            // uv_y: Default::default(),
            // color: vec4(1., 1., 1., 1.),
        }
    }
}

pub struct GeoSurface {
    pub start_index: u32,
    pub count: u32,
    // pub material: Rc<MaterialInstance>,

    // pub bounds: Bounds,
}

pub fn import_opti<P: AsRef<Path>>(obj_path: P) {
    let obj = parse_obj(BufReader::new(File::open(&obj_path).expect(&format!(
        "Couldn't load path : {}",
        obj_path.as_ref().to_string_lossy()
    ))))
    .expect("Couldn't load .obj");

    println!(
        "Loading object '{}' from path '{}' : {} polygons from {} points...",
        obj.name.unwrap_or("".to_string()),
        obj_path.as_ref().to_string_lossy(),
        obj.polygons.len(),
        obj.positions.len(),
    );

    let mut vertices = Vec::with_capacity(obj.positions.len());
    vertices.extend(obj.positions.iter().map(|(x, y, z, _)| Vertex {
        position: vec3(*x, *y, *z),
        ..Default::default()
    }));

    let mut indices: Vec<usize> = Vec::with_capacity(obj.polygons.len() * 3);
    for poly in obj.polygons.iter() {
        match poly {
            Polygon::P(vec) if vec.len() == 3 => indices.extend(vec.iter()),
            Polygon::PT(vec) | Polygon::PN(vec) if vec.len() == 3 => {
                indices.extend(vec.iter().map(|(p, _)| p))
            }
            Polygon::PTN(vec) if vec.len() == 3 => indices.extend(vec.iter().map(|(p, _, _)| p)),
            _ => panic!("Model should be triangulated first to be loaded properly"),
        }
    }

    let surfaces: Vec<_> = obj
        .meshes
        .iter()
        .flat_map(|(material_name, group)| {
            group.polygons.iter().map(|r| GeoSurface {
                start_index: (r.start * 3) as u32,
                count: dbg!(((r.end - r.start) * 3) as u32),
            })
        })
        .collect();

    // Split by mesh ?
    // println!("Groups : {}", obj.groups.keys().len());
    // obj.groups.keys().for_each(|k| println!("  - {k}"));
    //
    // TODO: hierarchy of nodes ?

    todo!();
}

// TODO: better error handling
pub fn import_triangles_and_diffuse<P: AsRef<Path>>(obj_path: P) -> Mesh {
    import_opti(&obj_path);

    let obj = parse_obj(BufReader::new(File::open(&obj_path).expect(&format!(
        "Couldn't load path : {}",
        obj_path.as_ref().to_string_lossy()
    ))))
    .expect("Couldn't load .obj");

    println!(
        "Loading object '{}' from path '{}' : {} polygons from {} points...",
        obj.name.unwrap_or("".to_string()),
        obj_path.as_ref().to_string_lossy(),
        obj.polygons.len(),
        obj.positions.len(),
    );

    let mtls = load_materials_diffuse_rgb(obj_path, &obj.material_libraries[..]);

    // TODO: différents groupes, materiaux on vetrices, ...
    let mut triangles = Vec::with_capacity(obj.polygons.len());
    for (poly_index, poly) in obj.polygons.iter().enumerate() {
        let texture = find_mtl_texture(&obj.meshes, &mtls, poly_index).unwrap_or_default();
        triangles.push(polygon_to_triangle(&obj.positions[..], texture, poly));
    }

    Mesh {
        triangles,
        ..Default::default()
    }
}

fn load_materials_diffuse_rgb<P: AsRef<Path>>(
    obj_path: P,
    mtl_librairies: &[String],
) -> HashMap<String, Texture> {
    let mut mtls = HashMap::new();
    for mtl_name in mtl_librairies.iter() {
        let path = obj_path
            .as_ref()
            .parent()
            .expect("Path should point to a file so have a parent !")
            .join(mtl_name);
        let mut mtl = parse_mtl(BufReader::new(
            File::open(&path).expect("Couldn't load path"),
        ))
        .expect("Couldn't load .mtl");

        println!(
            "Loading material '{}' : {} materials...",
            path.to_string_lossy(),
            mtl.materials.len(),
        );

        mtl.materials.drain().for_each(|(mtl_name, m)| {
            if let Some(MtlColor::Rgb(r, g, b)) = m.diffuse {
                mtls.insert(
                    mtl_name,
                    Texture::Color(
                        0xff000000
                            | (((r * 255.) as u32) << 16)
                            | (((g * 255.) as u32) << 8)
                            | ((b * 255.) as u32),
                    ),
                );
            } else {
                unimplemented!(
                    "Material {} with Non-RGB diffuse color {:?}",
                    mtl_name,
                    m.diffuse
                );
            }
        });
    }

    mtls
}

// TODO: remove two pubs
pub fn find_mtl_texture(
    meshes: &HashMap<String, Group>,
    materials: &HashMap<String, Texture>,
    polygon_index: usize,
) -> Option<Texture> {
    meshes
        .iter()
        .find_map(|(mtl_name, grp)| {
            grp.polygons
                .iter()
                .find(|range| polygon_index >= range.start && polygon_index < range.end)
                .map(|_| mtl_name)
        })
        .and_then(|mtl_name| materials.get(mtl_name).copied())
}

pub fn polygon_to_triangle(
    positions: &[(f32, f32, f32, f32)],
    texture: Texture,
    poly: &Polygon,
) -> Triangle {
    let map = |pos_index: usize| -> Vec3f {
        let (x, y, z, _) = positions[pos_index];
        Vec3f::new(x, y, z)
    };

    match poly {
        Polygon::P(vec) if vec.len() == 3 => {
            Triangle::new(map(vec[0]), map(vec[1]), map(vec[2]), texture)
        }
        Polygon::PT(vec) | Polygon::PN(vec) if vec.len() == 3 => {
            Triangle::new(map(vec[0].0), map(vec[1].0), map(vec[2].0), texture)
        }
        Polygon::PTN(vec) if vec.len() == 3 => {
            Triangle::new(map(vec[0].0), map(vec[1].0), map(vec[2].0), texture)
        }
        _ => panic!("Model should be triangulated first to be loaded properly"),
    }
}
