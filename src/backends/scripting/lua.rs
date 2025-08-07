use mlua::Lua;
use std::sync::Mutex;

use crate::api::ScriptingEngine;

pub struct LuaJitEngine {
    lua: Mutex<Lua>,
}

impl ScriptingEngine for LuaJitEngine {
    fn new() -> Self {
        Self { lua: Mutex::new(Lua::new()) }
    }
    
    fn exec(&self, code: &str) -> Result<(), String> {
        let lua = self.lua.lock().unwrap();
        lua.load(code).exec().map_err(|e| e.to_string())
    }

    fn set_global(&self, name: &str, val: &str) -> Result<(), String> {
        let lua = self.lua.lock().unwrap();
        lua.globals().set(name, val).map_err(|e| e.to_string())
    }
}