use mlua::Lua;
use std::sync::Mutex;

use crate::api::scripts::ScriptBackend;

pub struct LuaBackend {
    lua: Mutex<Lua>,
}

pub fn boxed() -> Box<dyn ScriptBackend> {
    Box::new(LuaBackend::new())
}

impl LuaBackend{
    pub fn new() -> Self {
        Self { lua: Mutex::new(Lua::new()) }
    }
}

impl ScriptBackend for LuaBackend {
    fn name(&self) -> &str {
        "lua"
    }

    fn exec(&mut self, code: &str) -> Result<(), String> {
        let lua = self.lua.lock().unwrap();
        lua.load(code).exec().map_err(|e| e.to_string())
    }
}
