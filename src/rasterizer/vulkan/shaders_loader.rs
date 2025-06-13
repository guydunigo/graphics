use std::{fs::File, io::Read, path::PathBuf, rc::Rc};

use ash::{Device, vk};
use shaderc::{CompilationArtifact, CompileOptions, Compiler, ShaderKind};

const SHADER_FOLDER: &str = "./resources/";
const SHADER_EXT: &str = "glsl";

/// Making it easy to load shaders from the [`SHADER_FOLDER`].
///
/// Known shaders are listed in the [`ShaderName`] enum.
/// Their corresponding name and kind are configured in the corresponding `From` impl for `&str` and
/// `ShaderKind`.
///
/// The file name should be `{name}.{kind}.glsl` : `colored_triangle_mesh.vert.glsl`
///
/// This struct stores a [`shaderc::Compiler`], which is costly to create.
pub struct ShadersLoader {
    device_copy: Rc<Device>,
    compiler: Compiler,
    // shaders: RefCell<HashMap<ShaderName, vk::ShaderModule>>,
}

impl ShadersLoader {
    pub fn new(device: Rc<Device>) -> Self {
        Self {
            device_copy: device,
            compiler: Compiler::new().unwrap(),
            // shaders: Default::default(),
        }
    }

    pub fn get(&self, name: ShaderName) -> ShaderModule {
        // let mut shaders = self.shaders.borrow_mut();
        // if shaders.contains_key(&name) {
        //     shaders.get(&name).unwrap()
        // } else {
        //     let module = self.load_shader_module(name);
        //     shaders.entry(name).or_insert(module)
        // }
        ShaderModule::load(self.device_copy.clone(), &self.compiler, name)
    }
}

// impl Drop for ShadersLoader {
//     fn drop(&mut self) {
//         #[cfg(feature = "dbg_mem")]
//         println!("drop VulkanShaders");
//         // println!("drop VulkanShaders : {} shaders", shaders.len());
//         // unsafe {
//         //     shaders
//         //         .drain()
//         //         .for_each(|(_, module)| self.device_copy.destroy_shader_module(module, None));
//         // }
//     }
// }

// TODO: stop using this and directly call with name + type ?
#[allow(dead_code)]
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum ShaderName {
    Gradient,
    ParametrableGradient,
    Sky,
    ColoredTriangleVert,
    ColoredTriangleFrag,
    ColoredTriangleMeshVert,
    TexImage,
    MeshFrag,
    MeshVert,
}

impl From<ShaderName> for &str {
    fn from(value: ShaderName) -> &'static str {
        use ShaderName::*;

        match value {
            Gradient => "gradient",
            ParametrableGradient => "parametrable_gradient",
            Sky => "sky",
            ColoredTriangleVert | ColoredTriangleFrag => "colored_triangle",
            ColoredTriangleMeshVert => "colored_triangle_mesh",
            TexImage => "tex_image",
            MeshFrag | MeshVert => "mesh",
        }
    }
}

impl From<ShaderName> for ShaderKind {
    fn from(value: ShaderName) -> ShaderKind {
        use ShaderKind::*;
        use ShaderName::*;

        match value {
            Gradient | ParametrableGradient | Sky => Compute,
            ColoredTriangleVert | ColoredTriangleMeshVert | MeshVert => Vertex,
            ColoredTriangleFrag | TexImage | MeshFrag => Fragment,
        }
    }
}

impl From<ShaderName> for PathBuf {
    fn from(value: ShaderName) -> Self {
        let name: &str = value.into();
        let stage = match value.into() {
            ShaderKind::Compute => "comp",
            ShaderKind::Vertex => "vert",
            ShaderKind::Fragment => "frag",
            _ => unimplemented!(),
        };
        let mut path = PathBuf::from(SHADER_FOLDER);
        path.push(format!("{name}.{stage}.{SHADER_EXT}"));
        path
    }
}

impl ShaderName {
    pub fn into_str(self) -> &'static str {
        self.into()
    }
}

/// Wrapper around a [`vk::ShaderModule`].
///
/// It will take care of destroying it on drop, so keep it around long enough.
pub struct ShaderModule {
    device_copy: Rc<Device>,
    // name: ShaderName,
    module: vk::ShaderModule,
}

impl ShaderModule {
    /// ShaderModule takes care of destroying the vk::ShaderModule on drop.
    /// So it **must** outlive this object, even if you copy it.
    pub fn module_copy(&self) -> vk::ShaderModule {
        self.module
    }

    /// Loads the corresponding `glsl` file and compiles it using `shaderc`.
    pub fn load(device: Rc<Device>, compiler: &Compiler, name: ShaderName) -> Self {
        let path: PathBuf = name.into();

        let mut glsl = String::new();
        File::open(path).unwrap().read_to_string(&mut glsl).unwrap();

        let spirv = Self::compile_glsl_to_spirv(compiler, name, &glsl);
        let create_info = vk::ShaderModuleCreateInfo::default().code(spirv.as_binary());

        let module = unsafe { device.create_shader_module(&create_info, None).unwrap() };

        Self {
            device_copy: device,
            // name,
            module,
        }
    }

    fn compile_glsl_to_spirv(
        compiler: &Compiler,
        name: ShaderName,
        glsl: &str,
    ) -> CompilationArtifact {
        let path: PathBuf = name.into();
        let mut options = CompileOptions::new().unwrap();
        options.set_include_callback(|name, include_type, _src_name, _| {
            let resolved_path = match include_type {
                shaderc::IncludeType::Relative => path.with_file_name(name),
                shaderc::IncludeType::Standard => {
                    let mut res = PathBuf::from(SHADER_FOLDER);
                    res.push(name);
                    res
                }
            };

            let mut content = String::new();
            File::open(resolved_path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();

            Ok(shaderc::ResolvedInclude {
                resolved_name: name.into(),
                content,
            })
        });
        let res =
            compiler.compile_into_spirv(glsl, name.into(), name.into_str(), "main", Some(&options));
        match res {
            Ok(res) => res,
            Err(shaderc::Error::CompilationError(nb, msg)) => {
                panic!(
                    "{nb} errors compiling shader `{}` :{}",
                    path.to_string_lossy(),
                    msg.lines()
                        .map(|s| format!("\n    - {s}"))
                        .collect::<String>()
                );
            }
            Err(e) => panic!("{e:?}"),
        }
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        {
            let kind: ShaderKind = self.name.into();
            println!("drop ShaderModule {:?}, kind {:?}", self.name, kind);
        }
        unsafe {
            self.device_copy.destroy_shader_module(self.module, None);
        }
    }
}

impl AsRef<vk::ShaderModule> for ShaderModule {
    fn as_ref(&self) -> &vk::ShaderModule {
        &self.module
    }
}
