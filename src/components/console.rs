use std::io::{self, Write};
use crate::api::scripting::ScriptingEngine;

/// A simple REPL-style developer console that uses a ScriptingEngine.
pub struct DeveloperConsole<E: ScriptingEngine> {
    engine: E,
}

impl<E: ScriptingEngine> DeveloperConsole<E> {
    /// Create a new developer console with the given scripting engine.
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// Start the REPL loop.
    pub fn run(&mut self) {
        println!("Developer Console started. Type 'exit' to quit.");
        println!("Enter code to execute. End input with a blank line to run.");

        let stdin = io::stdin();
        let mut input_buffer = String::new();

        loop {
            print!("> "); // friendly prompt
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if stdin.read_line(&mut line).is_err() {
                println!("Error reading input.");
                continue;
            }

            let trimmed = line.trim();

            // Exit command
            if trimmed.eq_ignore_ascii_case("exit") {
                println!("Exiting console.");
                break;
            }

            // Multi-line support: blank line runs the buffer
            if trimmed.is_empty() && !input_buffer.is_empty() {
                match self.engine.exec(&input_buffer) {
                    Ok(_) => println!("✅ Code executed successfully."),
                    Err(e) => println!("❌ Error: {}", e),
                }
                input_buffer.clear();
            } else {
                // Append line to buffer
                input_buffer.push_str(line.as_str());
            }
        }
    }
}