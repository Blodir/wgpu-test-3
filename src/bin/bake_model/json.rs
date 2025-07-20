pub mod model {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Primitive {
        pub instances: Vec<[[f32; 4]; 4]>,
        pub index_byte_offset: u32,
        pub index_byte_length: u32,
        pub material: u32,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Aabb {
        pub min: [f32; 3],
        pub max: [f32; 3],
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Model {
        pub primitives: Vec<Primitive>,
        pub materials: Vec<Material>,
        pub buffer_path: String,
        pub vertex_buffer_start_offset: u32,
        pub aabb: Aabb,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
    pub enum FilterMode {
        Nearest,
        Linear,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
    pub enum MipmapFilterMode {
        Nearest,
        Linear,
        None, // disables mipmapping
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
    pub enum WrapMode {
        Repeat,
        ClampToEdge,
        MirroredRepeat,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Sampler {
        pub mag_filter: FilterMode,
        pub min_filter: FilterMode,
        pub mipmap_filter: MipmapFilterMode,
        pub wrap_u: WrapMode,
        pub wrap_v: WrapMode,
        pub wrap_w: WrapMode,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Texture {
        pub source: String,
        pub sampler: Sampler,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum AlphaMode {
        Opaque,
        Mask,
        Blend,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Material {
        pub base_color_factor: [f32; 4],
        pub metallic_factor: f32,
        pub roughness_factor: f32,
        pub emissive_factor: [f32; 3],
        pub normal_texture_scale: f32,
        pub occlusion_strength: f32,
        pub alpha_mode: AlphaMode,
        pub alpha_cutoff: f32,
        pub double_sided: bool,
        pub normal_texture: Texture,
        pub occlusion_texture: Texture,
        pub emissive_texture: Texture,
        pub base_color_texture: Texture,
        pub metallic_roughness_texture: Texture,
    }
}
