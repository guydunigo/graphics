/// Describing the world...
use std::sync::{Arc, RwLock, Weak};

use rand::RngCore;

use crate::{
    maths::{PI, Rotation, Vec3f},
    scene::{Camera, Texture, Triangle},
};

#[derive(Default, Debug, Clone)]
pub struct ParTriangle {
    pub triangle: Triangle,
    pub mesh: Weak<RwLock<Mesh>>,
}

impl ParTriangle {
    /// Returns the projection of the triangle given the meshes position and rotation and scale.
    ///
    /// If the mesh isn't present, returns `None`.
    pub fn to_world(&self) -> Option<Triangle> {
        self.mesh.upgrade().map(|m| {
            let m = m.read().unwrap();
            self.triangle.scale_rot_move(m.scale, &m.rot, m.pos)
        })
    }
}

impl From<Triangle> for ParTriangle {
    fn from(triangle: Triangle) -> Self {
        ParTriangle {
            triangle,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mesh {
    triangles: Vec<Arc<ParTriangle>>,
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

impl Mesh {
    pub fn new() -> Arc<RwLock<Self>> {
        let mesh = Self::default();
        Arc::new(RwLock::new(mesh))
    }

    pub fn set_triangles(&mut self, me: &Arc<RwLock<Mesh>>, mut ts: Vec<Triangle>) {
        self.triangles = ts
            .drain(..)
            .map(|t| {
                let mut par_tri = ParTriangle::from(t);
                par_tri.mesh = Arc::downgrade(me);
                Arc::new(par_tri)
            })
            .collect();
    }
}

#[derive(Debug, Clone)]
pub struct World {
    meshes: Vec<Arc<RwLock<Mesh>>>,
    triangles: Vec<Weak<ParTriangle>>,
    pub camera: Camera,
    pub sun_direction: Vec3f,
}

impl World {
    pub fn meshes(&self) -> &[Arc<RwLock<Mesh>>] {
        &self.meshes
    }

    pub fn triangles(&self) -> &Vec<Weak<ParTriangle>> {
        &self.triangles
    }
}

impl Default for World {
    fn default() -> Self {
        let mut triangles = Vec::new();
        let meshes = vec![
            base_triangle(),
            base_pyramid(),
            obj::import_triangles_and_diffuse(obj::SUZANNE_OBJ_PATH),
            floor(),
            back_wall(),
            left_wall(),
            right_wall(),
        ]
        .drain(..)
        .inspect(|m| {
            m.read()
                .unwrap()
                .triangles
                .iter()
                .for_each(|t| triangles.push(Arc::downgrade(t)))
        })
        .collect();

        World {
            meshes,
            triangles,
            camera: Default::default(),
            sun_direction: Vec3f::new(-1., -1., -1.).normalize(),
        }
    }
}

mod obj {
    pub const SUZANNE_OBJ_PATH: &str = "resources/Suzanne.obj";

    use std::{
        collections::HashMap,
        fs::File,
        io::BufReader,
        path::Path,
        sync::{Arc, RwLock},
    };

    use obj::raw::{material::MtlColor, parse_mtl, parse_obj};

    use crate::{
        maths::Vec3f,
        scene::obj_file::{find_mtl_texture, polygon_to_triangle},
    };

    use super::{Mesh, Texture};

    // TODO: better error handling
    pub fn import_triangles_and_diffuse<P: AsRef<Path>>(obj_path: P) -> Arc<RwLock<Mesh>> {
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
            triangles.push(polygon_to_triangle(&obj.positions[..], texture, poly).into());
        }

        let res = Mesh::new();
        {
            let mut res_w = res.write().unwrap();
            res_w.set_triangles(&res, triangles);
            res_w.pos = Vec3f::new(0., 0., -10.);
        }
        res
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
}

fn base_triangle() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.set_triangles(&res, vec![Triangle::default()]);
        res_w.pos = Vec3f::new(0., 0., -10.);
    }
    res
}

fn base_pyramid() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.pos = Vec3f::new(4., 1., -19.);
        res_w.pos = Vec3f::new(4., 1., -19.);
        res_w.rot = Rotation::from_angles(0., 0., -PI / 3.);
        res_w.scale = 0.7;
        res_w.set_triangles(
            &res,
            vec![
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
        );
    }
    res
}

fn triangles_plane(color_mask: u32) -> Vec<Triangle> {
    const RANGE: i32 = 10;
    (-RANGE..RANGE)
        .flat_map(|x| {
            (-RANGE..RANGE)
                .map(move |z| {
                    (
                        Vec3f::new(x as f32, 0., z as f32),
                        rand::rng().next_u32() & color_mask,
                    )
                })
                .map(|(v, c)| {
                    Triangle::new(
                        v,
                        v + Vec3f::new(1., 0., 1.),
                        v + Vec3f::new(1., 0., 0.),
                        Texture::Color(c),
                    )
                })
        })
        .collect()
}

fn floor() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.set_triangles(&res, triangles_plane(0xff00ffff));
        res_w.pos = Vec3f::new(0., -10., 0.);
        res_w.scale = 5.;
    }
    res
}

fn back_wall() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.set_triangles(&res, triangles_plane(0xffffff00));
        res_w.pos = Vec3f::new(0., 0., -30.);
        res_w.rot = Rotation::from_angles(PI / 2., 0., 0.);
    }
    res
}

fn left_wall() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.set_triangles(&res, triangles_plane(0xffff00ff));
        res_w.pos = Vec3f::new(-10., 0., 0.);
        res_w.rot = Rotation::from_angles(0., 0., -PI / 2.);
    }
    res
}

fn right_wall() -> Arc<RwLock<Mesh>> {
    let res = Mesh::new();
    {
        let mut res_w = res.write().unwrap();
        res_w.set_triangles(&res, triangles_plane(0xffffffff));
        res_w.pos = Vec3f::new(10., 0., 0.);
        res_w.rot = Rotation::from_angles(0., 0., PI / 2.);
    }
    res
}
