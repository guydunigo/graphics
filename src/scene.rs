/// # Describing the world...
use crate::maths::{Rotation, Vec3f};

const DEFAULT_COLOR: u32 = 0xff999999;

#[derive(Debug, Clone, Copy)]
pub enum Texture {
    /// A simple color for the whole triangle
    Color(u32),
    /// A color per vertex in the same order :
    VertexColor(u32, u32, u32),
    // Texture, // TODO
}

impl Default for Texture {
    fn default() -> Self {
        Self::Color(DEFAULT_COLOR)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub p0: Vec3f,
    pub p1: Vec3f,
    pub p2: Vec3f,
    pub texture: Texture,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            p0: Vec3f::new(0., 1., -12.),
            p1: Vec3f::new(0., 0., -10.),
            p2: Vec3f::new(0., 0., -14.),
            texture: Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
        }
    }
}

impl Triangle {
    pub fn new(p0: Vec3f, p1: Vec3f, p2: Vec3f, texture: Texture) -> Self {
        Triangle {
            p0,
            p1,
            p2,
            texture,
        }
    }

    pub fn min_z(&self) -> f64 {
        f64::min(self.p0.z, f64::min(self.p1.z, self.p2.z))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f64,
    pub canvas_side: f64,
    pub rot: Rotation,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Vec3f::new(1., 1., 13.),
            z_near: 0.5,
            canvas_side: 0.1,
            rot: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct World {
    pub triangles: Vec<Triangle>,
    pub camera: Camera,
}

impl Default for World {
    fn default() -> Self {
        let mut triangles = vec![
            Triangle::default(),
            Triangle::new(
                Vec3f::new(3., 0., -19.),
                Vec3f::new(4., 0., -19.),
                Vec3f::new(4., 1., -10.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(4., 0., -19.),
                Vec3f::new(5., 0., -19.),
                Vec3f::new(4., 1., -10.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(3., 2., -19.),
                Vec3f::new(4., 1., -10.),
                Vec3f::new(4., 2., -19.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(4., 1., -10.),
                Vec3f::new(5., 2., -19.),
                Vec3f::new(4., 2., -19.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(3., 0., -19.),
                Vec3f::new(4., 1., -10.),
                Vec3f::new(3., 1., -19.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(3., 2., -19.),
                Vec3f::new(3., 1., -19.),
                Vec3f::new(4., 1., -10.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(5., 1., -19.),
                Vec3f::new(4., 1., -10.),
                Vec3f::new(5., 0., -19.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(4., 1., -10.),
                Vec3f::new(5., 1., -19.),
                Vec3f::new(5., 2., -19.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(2., 0.5, -19.),
                Vec3f::new(4., 0.5, -15.),
                Vec3f::new(2., 1.5, -19.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(4., 0.5, -15.),
                Vec3f::new(4., 1.5, -15.),
                Vec3f::new(2., 1.5, -19.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(3.7, 0.7, -12.),
                Vec3f::new(4.3, 0.7, -12.),
                Vec3f::new(3.7, 1.3, -12.),
                Texture::Color(0xffff00ff),
            ),
            Triangle::new(
                Vec3f::new(4.3, 0.7, -12.),
                Vec3f::new(4.3, 1.3, -12.),
                Vec3f::new(3.7, 1.3, -12.),
                Texture::Color(0xffff00ff),
            ),
        ];

        obj::import_triangles_and_diffuse(&mut triangles, obj::SUZANNE_OBJ_PATH);

        World {
            triangles,
            camera: Default::default(),
        }
    }
}

mod obj {
    pub const SUZANNE_OBJ_PATH: &str = "resources/Suzanne.obj";

    use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

    use obj::raw::{
        material::MtlColor,
        object::{Group, Polygon},
        parse_mtl, parse_obj,
    };

    use crate::maths::Vec3f;

    use super::{Texture, Triangle};

    // TODO: better error handling
    pub fn import_triangles_and_diffuse<P: AsRef<Path>>(
        triangles: &mut Vec<Triangle>,
        obj_path: P,
    ) {
        let obj = parse_obj(BufReader::new(
            File::open(&obj_path).expect("Couldn't load path"),
        ))
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
        for (poly_index, poly) in obj.polygons.iter().enumerate() {
            let texture =
                find_mtl_texture(&obj.meshes, &mtls, poly_index).unwrap_or(Default::default());
            triangles.push(polygon_to_triangle(&obj.positions[..], texture, poly));
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
                "Loading material '{}' from path '{}' : {} materials...",
                mtl_name,
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

    fn find_mtl_texture(
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

    fn polygon_to_triangle(
        positions: &[(f32, f32, f32, f32)],
        texture: Texture,
        poly: &Polygon,
    ) -> Triangle {
        let map = |pos_index: usize| -> Vec3f {
            let (x, y, z, _) = positions[pos_index];
            Vec3f::new(x as f64, y as f64, z as f64)
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
}
