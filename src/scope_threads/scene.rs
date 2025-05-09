use std::sync::{Arc, RwLock, Weak};

use rand::RngCore;

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

#[derive(Debug, Clone)]
pub struct Triangle {
    pub p0: Vec3f,
    pub p1: Vec3f,
    pub p2: Vec3f,
    pub texture: Texture,
    pub mesh: Weak<RwLock<Mesh>>,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            p0: Vec3f::new(0., 1., -2.),
            p1: Vec3f::new(0., 0., 0.),
            p2: Vec3f::new(0., 0., -4.),
            texture: Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
            mesh: Default::default(),
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
            ..Default::default()
        }
    }

    /* TODO
    pub fn min_z(&self) -> f32 {
        f32::min(self.p0.z, f32::min(self.p1.z, self.p2.z))
    }
    */

    fn scale_rot_move(&self, scale: f32, rot: &Rotation, move_vect: Vec3f) -> Self {
        Self {
            p0: self.p0.scale_rot_move(scale, rot, move_vect),
            p1: self.p1.scale_rot_move(scale, rot, move_vect),
            p2: self.p2.scale_rot_move(scale, rot, move_vect),
            texture: self.texture,
            ..Default::default()
        }
    }

    /// Returns the projection of the triangle given the meshes position and rotation and scale.
    ///
    /// If the mesh isn't present, returns `None`.
    pub fn to_world(&self) -> Option<Self> {
        self.mesh.upgrade().map(|m| {
            let m = m.read().unwrap();
            self.scale_rot_move(m.scale, &m.rot, m.pos)
        })
    }
}

#[derive(Debug, Clone)]
pub struct Mesh {
    triangles: Vec<Arc<Triangle>>,
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
            .map(|mut t| {
                t.mesh = Arc::downgrade(me);
                Arc::new(t)
            })
            .collect();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3f,
    pub z_near: f32,
    pub canvas_side: f32,
    /// Rotation matrix that will turn objects based on sight.
    /// It is made of opposite angles : if I turn to the left,
    /// the objects move to the right in my vision.
    ///
    /// This is the inverse of the actual rotation matrix of the camera "object" : `rot * sight_rot == identity`. See [`rot()`]
    ///
    /// We store this one because it is easier to manipulate and more used.
    sight_rot: Rotation,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Vec3f::new(1., 1., 12.),
            z_near: 0.5,
            canvas_side: 0.1,
            sight_rot: Default::default(),
        }
    }
}

impl Camera {
    const MOVE_STEP: f32 = 0.1;
    const ROT_STEP: f32 = 0.001;

    /// Rotation of the camera "object".
    ///
    /// The camera points towards `-rot().w` and the "up" is `rot().v`.
    ///
    /// It is calculated from [`sight_rot`] (inverse matrix), because it is only needed for
    /// movement.
    pub fn rot(&self) -> Rotation {
        self.sight_rot().inv()
    }

    pub fn sight_rot(&self) -> &Rotation {
        &self.sight_rot
    }

    pub fn reset_rot(&mut self) {
        self.sight_rot = Default::default();
    }

    pub fn rotate_from_mouse(&mut self, delta_x: f32, delta_y: f32) {
        // Objects rotate opposite direction from camera, so double negative.
        self.sight_rot = Rotation::from_angles(0., delta_x * Self::ROT_STEP, 0.)
            * &self.sight_rot
            * &Rotation::from_angles(delta_y * Self::ROT_STEP, 0., 0.);
    }

    /// Move along view direction
    /// `delta_x` : left->right
    /// `delta_y` : bottom->up
    /// `delta_z` : back->forward
    ///
    /// Z goes backwards so we reverse it.
    pub fn move_sight(&mut self, delta_x: f32, delta_y: f32, delta_z: f32) {
        let rot = self.rot();
        self.pos += (rot.u() * delta_x + rot.v() * delta_y - rot.w() * delta_z) * Self::MOVE_STEP;
    }

    pub fn world_to_sight(&self, point: Vec3f) -> Vec3f {
        (point - self.pos) * &self.sight_rot
    }
}

#[derive(Debug, Clone)]
pub struct World {
    meshes: Vec<Arc<RwLock<Mesh>>>,
    triangles: Vec<Weak<Triangle>>,
    pub camera: Camera,
    pub sun_direction: Vec3f,
}

impl World {
    pub fn meshes(&self) -> &[Arc<RwLock<Mesh>>] {
        &self.meshes
    }

    pub fn triangles(&self) -> &Vec<Weak<Triangle>> {
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

    use obj::raw::{
        material::MtlColor,
        object::{Group, Polygon},
        parse_mtl, parse_obj,
    };

    use crate::maths::Vec3f;

    use super::{Mesh, Texture, Triangle};

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
            triangles.push(polygon_to_triangle(&obj.positions[..], texture, poly));
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
