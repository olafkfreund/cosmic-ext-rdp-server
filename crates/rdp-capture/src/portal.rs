use std::os::fd::OwnedFd;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType, Stream as ScreencastStream};
use ashpd::desktop::PersistMode;

/// Information about a captured screen stream.
#[derive(Debug, Clone)]
pub struct PortalStream {
    /// `PipeWire` node ID to connect to.
    pub node_id: u32,
    /// Stream width reported by the portal (compositor logical coordinates).
    pub width: Option<i32>,
    /// Stream height reported by the portal (compositor logical coordinates).
    pub height: Option<i32>,
}

impl From<&ScreencastStream> for PortalStream {
    fn from(stream: &ScreencastStream) -> Self {
        let (width, height) = stream.size().map_or((None, None), |(w, h)| (Some(w), Some(h)));
        Self {
            node_id: stream.pipe_wire_node_id(),
            width,
            height,
        }
    }
}

/// Result of starting a screen capture session via the portal.
pub struct PortalSession {
    /// The active portal session (must be kept alive).
    pub session: ashpd::desktop::Session<'static, Screencast<'static>>,
    /// The proxy (must be kept alive).
    pub proxy: Screencast<'static>,
    /// Streams available for capture.
    pub streams: Vec<PortalStream>,
    /// Restore token for persistent sessions.
    pub restore_token: Option<String>,
    /// `PipeWire` file descriptor for connecting.
    pub pipewire_fd: OwnedFd,
}

/// Start a `ScreenCast` portal session and get a `PipeWire` connection.
///
/// This will show the system permission dialog if no valid restore token is provided.
///
/// # Errors
///
/// Returns `PortalError` if the portal session cannot be created or started.
pub async fn start_screencast(
    restore_token: Option<&str>,
) -> Result<PortalSession, PortalError> {
    let proxy = Screencast::new().await.map_err(PortalError::Create)?;

    let session = proxy
        .create_session()
        .await
        .map_err(PortalError::Session)?;

    proxy
        .select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor.into(),
            false,
            restore_token,
            PersistMode::ExplicitlyRevoked,
        )
        .await
        .map_err(PortalError::SelectSources)?;

    let response = proxy
        .start(&session, None)
        .await
        .map_err(PortalError::Start)?
        .response()
        .map_err(PortalError::Response)?;

    let streams: Vec<PortalStream> = response
        .streams()
        .iter()
        .map(PortalStream::from)
        .collect();

    if streams.is_empty() {
        return Err(PortalError::NoStreams);
    }

    let restore_token = response.restore_token().map(String::from);

    let pipewire_fd = proxy
        .open_pipe_wire_remote(&session)
        .await
        .map_err(PortalError::PipeWireRemote)?;

    tracing::info!(
        node_id = streams[0].node_id,
        width = ?streams[0].width,
        height = ?streams[0].height,
        streams = streams.len(),
        "ScreenCast portal session started"
    );

    Ok(PortalSession {
        session,
        proxy,
        streams,
        restore_token,
        pipewire_fd,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("failed to create ScreenCast proxy")]
    Create(#[source] ashpd::Error),

    #[error("failed to create session")]
    Session(#[source] ashpd::Error),

    #[error("failed to select sources")]
    SelectSources(#[source] ashpd::Error),

    #[error("failed to start session")]
    Start(#[source] ashpd::Error),

    #[error("user cancelled or portal response failed")]
    Response(#[source] ashpd::Error),

    #[error("no streams returned by portal")]
    NoStreams,

    #[error("failed to open PipeWire remote")]
    PipeWireRemote(#[source] ashpd::Error),
}
