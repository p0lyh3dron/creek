//! This modules provides the Graphics API.
//! 
//! It allows abstraction over different graphics backends
//! such as Vulkan, software, etc.

/// A trait for backend-agnostic 3D graphics rendering.
///
/// This trait provides basic methods for initializing a graphics pipeline,
/// submitting frames, and handling resource lifetimes.
pub trait Graphics {
    /// Initializes the graphics context (e.g., Vulkan instance, OpenGL context).
    fn initialize() -> Self
    where
        Self: Sized;

    /// Begins rendering a new frame.
    fn begin_frame(&mut self);

    /// Ends the current frame and presents it to the screen.
    fn end_frame(&mut self);

    /// Whether the graphics context is still active.
    fn is_active(&self) -> bool;
}