pub mod api;
pub mod backends;
pub mod prelude;

#[cfg(feature = "desktop")]
pub use backends::window::desktop::DesktopWindow as Window;

#[cfg(feature = "embedded")]
pub use backends::window::embedded::EmbeddedWindow as Window;

#[cfg(feature = "vulkan")]
pub use backends::graphics::vulkan::VulkanGraphics as Graphics;