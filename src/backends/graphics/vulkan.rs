use crate::api::Graphics;

/// A Vulkan-based implementation of the `Graphics` trait.
///
/// Uses ash or erupt (Vulkan bindings) under the hood.
pub struct VulkanGraphics {
    initialized: bool,
}

impl Graphics for VulkanGraphics {
    fn initialize() -> Self {
        // Placeholder for Vulkan init
        println!("Initializing Vulkan...");
        Self { initialized: true }
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