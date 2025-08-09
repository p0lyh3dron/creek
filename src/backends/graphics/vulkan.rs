use crate::api::Graphics;

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk, Device, Entry, Instance,
};

/// A Vulkan-based implementation of the `Graphics` trait.
///
/// Uses ash or erupt (Vulkan bindings) under the hood.
pub struct VulkanGraphics {
    initialized: bool,

    pub instance: Instance,
}

impl Graphics for VulkanGraphics {
    fn initialize() -> Self {
        // Placeholder for Vulkan init
        println!("Initializing Vulkan...");
        Self { initialized: true, instance: 0 }
    }

    fn test(&self) {

    }

    fn begin_frame(&mut self) {
        println!("Beginning Vulkan frame...");
    }

    fn end_frame(&mut self) {
        println!("Ending Vulkan frame...");
    }

    fn is_active(&self) -> bool {
        self.initialized
    }
}