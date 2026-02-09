use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use pipewire as pw;
use pw::properties::properties;
use pw::stream::{Stream, StreamFlags, StreamState};
use tokio::sync::mpsc;

use crate::frame::{CapturedFrame, DamageRect, PixelFormat};

/// Handle to a running `PipeWire` capture stream.
///
/// The stream runs on a dedicated OS thread with its own `PipeWire` `MainLoop`.
/// Frames are delivered via a tokio mpsc channel.
pub struct PwStream {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PwStream {
    /// Start capturing from the given `PipeWire` node using the portal's fd.
    ///
    /// Returns a `PwStream` handle and a receiver for captured frames.
    ///
    /// # Errors
    ///
    /// Returns `PwError` if the `PipeWire` thread cannot be spawned.
    pub fn start(
        pipewire_fd: OwnedFd,
        node_id: u32,
        channel_capacity: usize,
    ) -> Result<(Self, mpsc::Receiver<CapturedFrame>), PwError> {
        let (tx, rx) = mpsc::channel(channel_capacity);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let thread = std::thread::Builder::new()
            .name("pw-capture".into())
            .spawn(move || {
                if let Err(e) = run_pipewire_loop(pipewire_fd, node_id, tx, running_clone) {
                    tracing::error!("PipeWire thread exited with error: {e}");
                }
            })
            .map_err(PwError::SpawnThread)?;

        Ok((
            Self {
                running,
                thread: Some(thread),
            },
            rx,
        ))
    }

    /// Stop the `PipeWire` stream and join the thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PwStream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run the `PipeWire` main loop on a dedicated thread.
#[allow(clippy::needless_pass_by_value)] // Arc is moved from a thread spawn closure
fn run_pipewire_loop(
    pipewire_fd: OwnedFd,
    node_id: u32,
    frame_tx: mpsc::Sender<CapturedFrame>,
    running: Arc<AtomicBool>,
) -> Result<(), PwError> {
    pw::init();

    let mainloop = pw::main_loop::MainLoop::new(None).map_err(|_| PwError::MainLoop)?;
    let context = pw::context::Context::new(&mainloop).map_err(|_| PwError::Context)?;
    let core = context
        .connect_fd(pipewire_fd, None)
        .map_err(|_| PwError::ConnectFd)?;

    let stream = Stream::new(
        &core,
        "cosmic-rdp-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(|_| PwError::CreateStream)?;

    let seq = Arc::new(AtomicU64::new(0));

    let _listener = stream
        .add_local_listener_with_user_data(frame_tx)
        .state_changed(|_stream, _tx, old, new| {
            tracing::debug!("PipeWire stream state: {old:?} -> {new:?}");
            if new == StreamState::Error(String::new()) {
                tracing::error!("PipeWire stream entered error state");
            }
        })
        .process(move |stream_ref, tx| {
            process_frame(stream_ref, tx, &seq);
        })
        .register()
        .map_err(|_| PwError::RegisterListener)?;

    stream
        .connect(
            pw::spa::utils::Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut [],
        )
        .map_err(|_| PwError::StreamConnect)?;

    tracing::info!(node_id, "PipeWire stream connected, entering main loop");

    while running.load(Ordering::SeqCst) {
        mainloop.loop_().iterate(std::time::Duration::from_millis(50));
    }

    tracing::info!("PipeWire main loop exiting");
    Ok(())
}

/// Process a single frame from the `PipeWire` stream.
fn process_frame(
    stream: &pw::stream::StreamRef,
    tx: &mut mpsc::Sender<CapturedFrame>,
    seq: &AtomicU64,
) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };

    let datas = buffer.datas_mut();
    if datas.is_empty() {
        return;
    }

    let data = &mut datas[0];

    // Read chunk metadata before taking the mutable data borrow.
    let chunk = data.chunk();
    #[allow(clippy::cast_sign_loss)] // negative stride is invalid, treated as zero below
    let stride = chunk.stride() as u32;
    let offset = chunk.offset() as usize;
    let size = chunk.size() as usize;

    let Some(slice) = data.data() else {
        return;
    };

    if size == 0 || stride == 0 {
        return;
    }

    // Infer dimensions from stride and size.
    // PipeWire BGRx/BGRA is 4 bytes per pixel.
    let bpp = 4u32;
    let width = stride / bpp;
    #[allow(clippy::cast_possible_truncation)] // frame size always fits in u32
    let height = if stride > 0 { (size as u32) / stride } else { 0 };

    if width == 0 || height == 0 {
        return;
    }

    let end = offset + size;
    if end > slice.len() {
        tracing::warn!(
            offset,
            size,
            slice_len = slice.len(),
            "Buffer slice out of bounds"
        );
        return;
    }

    let frame_data = slice[offset..end].to_vec();
    let sequence = seq.fetch_add(1, Ordering::Relaxed);

    // Extract damage rects from SPA metadata (unsafe FFI).
    let damage = extract_damage(stream);

    let frame = CapturedFrame {
        data: frame_data,
        width,
        height,
        format: PixelFormat::Bgra,
        stride,
        sequence,
        damage,
    };

    // Non-blocking send. Drop frame if channel is full to avoid backpressure.
    if tx.try_send(frame).is_err() {
        tracing::trace!("Frame channel full, dropping frame {sequence}");
    }
}

/// Extract damage rectangles from `PipeWire` buffer metadata.
///
/// Uses the raw `pw_buffer` to access SPA metadata. Returns `None` if no
/// damage metadata is present.
fn extract_damage(stream: &pw::stream::StreamRef) -> Option<Vec<DamageRect>> {
    // The safe `dequeue_buffer()` API doesn't expose raw SPA metadata.
    // For now, return None (full frame damage) which is correct but
    // less efficient. Damage extraction will be added when we optimize
    // bandwidth with partial updates.
    //
    // TODO: Use unsafe raw buffer access to parse SPA_META_VideoDamage
    let _ = stream;
    None
}

#[derive(Debug, thiserror::Error)]
pub enum PwError {
    #[error("failed to create PipeWire MainLoop")]
    MainLoop,

    #[error("failed to create PipeWire Context")]
    Context,

    #[error("failed to connect to PipeWire via portal fd")]
    ConnectFd,

    #[error("failed to create PipeWire Stream")]
    CreateStream,

    #[error("failed to register stream listener")]
    RegisterListener,

    #[error("failed to connect stream to node")]
    StreamConnect,

    #[error("failed to spawn PipeWire thread")]
    SpawnThread(#[source] std::io::Error),
}
