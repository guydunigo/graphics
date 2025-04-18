/// # Describing the world...
use crate::maths::{PI, Rotation, Vec3f};

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
            p0: Vec3f::new(0., 1., -2.),
            p1: Vec3f::new(0., 0., 0.),
            p2: Vec3f::new(0., 0., -4.),
            texture: Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
        }
    }
}

impl Triangle {
    pub const fn new(p0: Vec3f, p1: Vec3f, p2: Vec3f, texture: Texture) -> Self {
        Triangle {
            p0,
            p1,
            p2,
            texture,
        }
    }

    pub fn min_z(&self) -> f32 {
        f32::min(self.p0.z, f32::min(self.p1.z, self.p2.z))
    }

    pub fn scale_rot_move(&self, scale: f32, rot: &Rotation, move_vect: Vec3f) -> Triangle {
        Triangle {
            p0: self.p0.scale_rot_move(scale, rot, move_vect),
            p1: self.p1.scale_rot_move(scale, rot, move_vect),
            p2: self.p2.scale_rot_move(scale, rot, move_vect),
            texture: self.texture,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
    pub pos: Vec3f,
    pub rot: Rotation,
    pub scale: f32,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            triangles: Default::default(),
            pos: Default::default(),
            rot: Default::default(),
            scale: 1.,
        }
    }
}

impl From<Triangle> for Mesh {
    fn from(value: Triangle) -> Self {
        Mesh {
            triangles: vec![value],
            ..Default::default()
        }
    }
}

impl Mesh {
    pub fn with_translation_to(self, new_pos: Vec3f) -> Self {
        Self {
            pos: new_pos,
            ..self
        }
    }

    pub fn to_world_triangles(&self) -> impl Iterator<Item = Triangle> {
        self.triangles
            .iter()
            .map(|t| t.scale_rot_move(self.scale, &self.rot, self.pos))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f32,
    pub canvas_side: f32,
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

impl Camera {
    pub fn rotate_from_mouse(&mut self, delta_x: f32, delta_y: f32) {
        self.rot = Rotation::from_angles(0., delta_x as f32 * 0.001, 0.)
            * &self.rot
            * &Rotation::from_angles(delta_y as f32 * 0.001, 0., 0.);
    }
}

#[derive(Debug, Clone)]
pub struct World {
    pub meshes: Vec<Mesh>,
    pub camera: Camera,
}

impl Default for World {
    fn default() -> Self {
        World {
            meshes: vec![
                Mesh::from(Triangle::default()).with_translation_to(Vec3f::new(0., 0., -10.)),
                base_pyramid(),
                obj::import_triangles_and_diffuse(obj::SUZANNE_OBJ_PATH),
            ],
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

    use super::{Mesh, Texture, Triangle};

    // TODO: better error handling
    pub fn import_triangles_and_diffuse<P: AsRef<Path>>(obj_path: P) -> Mesh {
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
}

fn base_pyramid() -> Mesh {
    Mesh {
        triangles: vec![
            Triangle::new(
                Vec3f::new(-1., -1., 0.),
                Vec3f::new(0., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(0., -1., 0.),
                Vec3f::new(1., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(-1., 1., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(0., 1., 0.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., 1., 0.),
                Vec3f::new(0., 1., 0.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(-1., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(-1., 0., 0.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(-1., 1., 0.),
                Vec3f::new(-1., 0., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(1., 0., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., -1., 0.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., 0., 0.),
                Vec3f::new(1., 1., 0.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(-2., -0.5, 0.),
                Vec3f::new(0., -0.5, 4.),
                Vec3f::new(-2., 0.5, 0.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(0., -0.5, 4.),
                Vec3f::new(0., 0.5, 4.),
                Vec3f::new(-2., 0.5, 0.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(-0.3, -0.3, 7.),
                Vec3f::new(0.3, -0.3, 7.),
                Vec3f::new(-0.3, 0.3, 7.),
                Texture::Color(0xffff00ff),
            ),
            Triangle::new(
                Vec3f::new(0.3, -0.3, 7.),
                Vec3f::new(0.3, 0.3, 7.),
                Vec3f::new(-0.3, 0.3, 7.),
                Texture::Color(0xffff00ff),
            ),
        ],
        pos: Vec3f {
            x: 4.,
            y: 1.,
            z: -19.,
        },
        rot: Rotation::from_angles(0., 0., -PI / 3.),
        scale: 0.7,
    }
}
