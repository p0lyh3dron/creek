//! This modules provides the Scripting API.
//! 
//! It allows abstraction over different scripting backends
//! such as Lua, Python, etc.

/// A trait for backend-agnostic scripting support.
/// 
/// This trait provides basic methods for initializing a scripting environment,
/// executing scripts, and handling errors.
use std::fmt::Debug;

pub trait ScriptingEngine: {
    /// Initialize the scripting engine.
    fn new() -> Self
    where
        Self: Sized;

    /// Execute arbitrary code. No return value expected.
    fn exec(&self, code: &str) -> Result<(), String>;

    /// Set a global variable from host application.
    fn set_global(&self, name: &str, val: &str) -> Result<(), String>;
}