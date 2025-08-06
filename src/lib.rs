pub mod api;
pub mod backends;
pub mod prelude;

#[cfg(feature = "desktop")]
pub use backends::desktop::DesktopWindow as Window;

#[cfg(feature = "embedded")]
pub use backends::embedded::EmbeddedWindow as Window;