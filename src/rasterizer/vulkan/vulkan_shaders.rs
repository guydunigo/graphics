use std::{cell::RefCell, collections::HashMap, fs::File, io::Read, path::PathBuf, rc::Rc};

use ash::{Device, vk};
use naga::{ShaderStage, back::spv, front::glsl, valid::Validator};

const SHADER_FOLDER: &str = "./resources/";
const SHADER_EXT: &str = "glsl";

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum ShaderName {
    Gradient,
    ParametrableGradient,
}

impl Into<&str> for ShaderName {
    fn into(self) -> &'static str {
        use ShaderName::*;

        match self {
            Gradient => "gradient",
            ParametrableGradient => "parametrable_gradient",
        }
    }
}

impl Into<PathBuf> for ShaderName {
    fn into(self) -> PathBuf {
        let name: &str = self.into();
        let mut path = PathBuf::from(SHADER_FOLDER);
        path.push(name);
        path.set_extension(SHADER_EXT);

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
                glsl_parser: glsl::Frontend::default(),
                validator: naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                ),

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
    glsl_parser: glsl::Frontend,
    validator: Validator,
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
        let spirv = self.compile_glsl_to_spirv(&glsl);

        let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv[..]);
        unsafe {
            self.device_copy
                .create_shader_module(&create_info, None)
                .unwrap()
        }
    }

    fn compile_glsl_to_spirv(&mut self, glsl: &str) -> Vec<u32> {
        let module = {
            let options = glsl::Options::from(ShaderStage::Compute);
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
