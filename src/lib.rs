pub mod api;
pub mod backends;
pub mod components;
pub mod prelude;

pub use components::console::DeveloperConsole as Console;

#[cfg(feature = "desktop")]
pub use backends::window::desktop::DesktopWindow as Window;

#[cfg(feature = "embedded")]
pub use backends::window::embedded::EmbeddedWindow as Window;

#[cfg(feature = "vulkan")]
pub use backends::graphics::vulkan::VulkanGraphics as Graphics;

#[cfg(feature = "lua")]
pub use backends::scripting::lua::LuaJitEngine as ScriptingEngine;