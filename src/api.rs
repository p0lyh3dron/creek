//! This module provides the API for Creek's functionality.
//!
//! It allows abstraction over desktop and embedded environments
//! using conditional compilation and a unified API trait.

/// A window abstraction used to provide a unified API across backends.
///
/// The `Window` trait allows backend-specific implementations to be swapped
/// at compile-time depending on platform or environment.
///
/// # Example
///
/// ```rust
/// let mut window = MyWindow::new("Hello", 800, 600);
/// while window.is_open() {
///     window.update();
/// }
/// ```
pub trait Window {
    /// Create a new window with the given title and size.
    ///
    /// # Arguments
    ///
    /// * `title` - The title of the window.
    /// * `width` - The width in pixels.
    /// * `height` - The height in pixels.
    fn new(title: &str, width: u32, height: u32) -> Self
    where
        Self: Sized;

    /// Process the event queue and update the window state.
    fn update(&mut self);

    /// Check whether the window is still open.
    fn is_open(&self) -> bool;
}