use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Current status of the RDP server daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[repr(u8)]
pub enum ServerStatus {
    /// Server is stopped / not running.
    Stopped = 0,
    /// Server is starting up.
    Starting = 1,
    /// Server is running and accepting connections.
    Running = 2,
    /// Server encountered an error.
    Error = 3,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Information about a connected RDP client.
///
/// Reserved for future use when client connection tracking is implemented.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[allow(dead_code)]
pub struct ClientInfo {
    /// Remote address of the client.
    pub address: String,
    /// Unix timestamp (seconds) when the client connected.
    pub connected_at: i64,
}

/// State of a broker-managed user session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[repr(u8)]
pub enum SessionState {
    /// Session is being spawned (server starting).
    Starting = 0,
    /// Session is active (server running, may or may not have a client).
    Active = 1,
    /// Session is idle (client disconnected, awaiting timeout or reconnect).
    Idle = 2,
    /// Session is being terminated.
    Stopping = 3,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Active => write!(f, "Active"),
            Self::Idle => write!(f, "Idle"),
            Self::Stopping => write!(f, "Stopping"),
        }
    }
}

/// Information about a broker-managed user session.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SessionInfo {
    /// Unix username.
    pub username: String,
    /// Port the per-user server is listening on.
    pub port: u16,
    /// Process ID of the per-user server.
    pub pid: u32,
    /// Session state.
    pub state: SessionState,
    /// Unix timestamp (seconds) when the session was created.
    pub created_at: i64,
    /// Remote address of the most recent client (empty if none).
    pub client_addr: String,
}
