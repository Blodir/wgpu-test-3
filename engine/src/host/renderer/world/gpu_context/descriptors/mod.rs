pub mod bind_groups;
pub mod texture_views;
pub mod bg_layouts;

pub(crate) use bind_groups::*;
pub(crate) use texture_views::*;

pub(crate) struct Descriptors {
    pub(crate) bind_groups: BindGroups,
    pub(crate) bind_group_layouts: bg_layouts::BGLayouts,
    pub(crate) texture_views: TextureViews,
}
