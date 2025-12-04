use serde::{Deserialize, Serialize};

/// All supported commands for the fill station
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Ignite,
}

/// Response sent back to WebSocket clients after command execution
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResponse {
    Success,
    Error,
}
