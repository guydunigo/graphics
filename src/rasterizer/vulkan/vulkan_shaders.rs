use std::{cell::RefCell, collections::HashMap, fs::File, io::Read, path::PathBuf, rc::Rc};

use ash::{Device, vk};

#[cfg(feature = "naga")]
use naga::{ShaderStage, back::spv, front::glsl, valid::Validator};

const SHADER_FOLDER: &str = "./resources/";
const SHADER_EXT: &str = "glsl";

// TODO: shaders can be destroyed once pipeline is created, should we keep them ? or just keep
// compiled code ?

#[allow(dead_code)]
enum ShaderStage {
    Task,
    Mesh,
    Vertex,
    Compute,
    Fragment,
}

#[cfg(feature = "naga")]
impl Into<naga::ShaderStage> for ShaderStage {
    fn into(self) -> naga::ShaderStage {
        match self {
            ShaderStage::Task => naga::ShaderStage::Task,
            ShaderStage::Mesh => naga::ShaderStage::Mesh,
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Compute => naga::ShaderStage::Compute,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
        }
    }
}

#[cfg(not(feature = "naga"))]
impl Into<shaderc::ShaderKind> for ShaderStage {
    fn into(self) -> shaderc::ShaderKind {
        match self {
            ShaderStage::Task => shaderc::ShaderKind::Task,
            ShaderStage::Mesh => shaderc::ShaderKind::Mesh,
            ShaderStage::Vertex => shaderc::ShaderKind::Vertex,
            ShaderStage::Compute => shaderc::ShaderKind::Compute,
            ShaderStage::Fragment => shaderc::ShaderKind::Fragment,
        }
    }
}

#[allow(dead_code)]
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum ShaderName {
    Gradient,
    ParametrableGradient,
    Sky,
    ColoredTriangleVert,
    ColoredTriangleFrag,
    ColoredTriangleMeshVert,
}

impl ShaderName {
    pub fn into_str(self) -> &'static str {
        self.into()
    }
}

impl Into<&str> for ShaderName {
    fn into(self) -> &'static str {
        use ShaderName::*;

        match self {
            Gradient => "gradient",
            ParametrableGradient => "parametrable_gradient",
            Sky => "sky",
            ColoredTriangleVert | ColoredTriangleFrag => "colored_triangle",
            ColoredTriangleMeshVert => "colored_triangle_mesh",
        }
    }
}

impl Into<ShaderStage> for ShaderName {
    fn into(self) -> ShaderStage {
        use ShaderName::*;
        use ShaderStage::*;

        match self {
            Gradient | ParametrableGradient | Sky => Compute,
            ColoredTriangleVert => Vertex,
            ColoredTriangleFrag => Fragment,
            ColoredTriangleMeshVert => Vertex,
        }
    }
}

impl Into<PathBuf> for ShaderName {
    fn into(self) -> PathBuf {
        let name: &str = self.into();
        let stage = match self.into() {
            ShaderStage::Compute => "comp",
            ShaderStage::Vertex => "vert",
            ShaderStage::Fragment => "frag",
            _ => unimplemented!(),
        };
        let mut path = PathBuf::from(SHADER_FOLDER);
        path.push(format!("{}.{}.{}", name, stage, SHADER_EXT));
        path
    }
}

pub struct VulkanShaders {
    inner: RefCell<VulkanShadersMutable>,
}

impl VulkanShaders {
    pub fn new(device: Rc<Device>) -> Self {
        Self {
            inner: RefCell::new(VulkanShadersMutable {
                device_copy: device,
                #[cfg(feature = "naga")]
                glsl_parser: glsl::Frontend::default(),
                #[cfg(feature = "naga")]
                validator: naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                ),
                #[cfg(not(feature = "naga"))]
                compiler: shaderc::Compiler::new().unwrap(),

                shaders: Default::default(),
            }),
        }
    }

    pub fn get(&self, name: ShaderName) -> vk::ShaderModule {
        let mut inner = self.inner.borrow_mut();
        *inner.get(name)
    }
}

struct VulkanShadersMutable {
    device_copy: Rc<Device>,

    #[cfg(feature = "naga")]
    glsl_parser: glsl::Frontend,
    #[cfg(feature = "naga")]
    validator: Validator,
    #[cfg(not(feature = "naga"))]
    compiler: shaderc::Compiler,

    shaders: HashMap<ShaderName, vk::ShaderModule>,
}

impl VulkanShadersMutable {
    pub fn get(&mut self, name: ShaderName) -> &vk::ShaderModule {
        if self.shaders.contains_key(&name) {
            self.shaders.get(&name).unwrap()
        } else {
            let module = self.load_shader_module(name);
            self.shaders.entry(name).or_insert(module)
        }
    }

    fn load_shader_module(&mut self, name: ShaderName) -> vk::ShaderModule {
        let path: PathBuf = name.into();

        let mut glsl = String::new();
        File::open(path).unwrap().read_to_string(&mut glsl).unwrap();

        #[cfg(feature = "naga")]
        let create_info = {
            let spirv = self.compile_glsl_to_spirv(&glsl, name.into());
            vk::ShaderModuleCreateInfo::default().code(&spirv[..])
        };
        #[cfg(not(feature = "naga"))]
        let spirv = self.compile_glsl_to_spirv(name, &glsl);
        #[cfg(not(feature = "naga"))]
        let create_info = { vk::ShaderModuleCreateInfo::default().code(spirv.as_binary()) };

        unsafe {
            self.device_copy
                .create_shader_module(&create_info, None)
                .unwrap()
        }
    }

    #[cfg(feature = "naga")]
    fn compile_glsl_to_spirv(&mut self, glsl: &str, stage: ShaderStage) -> Vec<u32> {
        let module = {
            let options = glsl::Options::from(stage);
            self.glsl_parser.parse(&options, &glsl).unwrap()
        };

        let module_info: naga::valid::ModuleInfo = self
            .validator
            .subgroup_stages(naga::valid::ShaderStages::all())
            .subgroup_operations(naga::valid::SubgroupOperationSet::all())
            .validate(&module)
            .unwrap();

        let options = spv::Options::default();
        spv::write_vec(&module, &module_info, &options, None).unwrap()
    }

    #[cfg(not(feature = "naga"))]
    fn compile_glsl_to_spirv(&self, name: ShaderName, glsl: &str) -> shaderc::CompilationArtifact {
        let mut options = shaderc::CompileOptions::new().unwrap();
        options.add_macro_definition("EP", Some("main"));
        let stage: ShaderStage = name.into();
        self.compiler
            .compile_into_spirv(glsl, stage.into(), name.into_str(), "main", Some(&options))
            .unwrap()
    }
}

impl Drop for VulkanShadersMutable {
    fn drop(&mut self) {
        println!("drop VulkanShaders : {} shaders", self.shaders.len());
        unsafe {
            self.shaders
                .drain()
                .for_each(|(_, module)| self.device_copy.destroy_shader_module(module, None));
        }
    }
}
