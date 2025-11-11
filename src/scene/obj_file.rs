pub const SUZANNE_OBJ_PATH: &str = "resources/Suzanne.obj";

use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use glam::vec3;
use obj::raw::{material::MtlColor, object::Polygon, parse_mtl, parse_obj};

use super::{GeoSurface, MeshAsset, Texture, Vertex};

// TODO: better error handling
pub fn import_mesh_and_diffuse<P: AsRef<Path>>(obj_path: P) -> MeshAsset {
    let obj = parse_obj(BufReader::new(File::open(&obj_path).unwrap_or_else(|_| panic!("Couldn't load path : {}",
        obj_path.as_ref().to_string_lossy()))))
    .expect("Couldn't load .obj");

    println!(
        "Loading object '{}' from path '{}' : {} polygons from {} points...",
        obj.name.unwrap_or("".to_string()),
        obj_path.as_ref().to_string_lossy(),
        obj.polygons.len(),
        obj.positions.len(),
    );

    let mtls = load_materials_diffuse_rgb(obj_path, &obj.material_libraries[..]);

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
    // TODO PTN save normal

    let surfaces: Vec<_> = obj
        .meshes
        .iter()
        .flat_map(|(material_name, group)| {
            group.polygons.iter().map(|r| {
                let range = (r.start * 3)..(r.end * 3);
                GeoSurface::new(
                    &vertices[..],
                    &indices[..],
                    r.start * 3,
                    range.len(),
                    mtls[material_name],
                )
            })
        })
        .collect();

    // Split by mesh ?
    // println!("Groups : {}", obj.groups.keys().len());
    // obj.groups.keys().for_each(|k| println!("  - {k}"));
    // TODO: hierarchy of nodes ?

    MeshAsset::new(vertices, indices, surfaces)
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
