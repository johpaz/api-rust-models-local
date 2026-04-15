use std::sync::{mpsc, Arc};
use std::thread;
use anyhow::Result;
use tracing::{debug, warn};
use crate::layer_loader::{LayerLoader, LayerWeights};

/// Request sent to the background loader thread.
enum PrefetchRequest {
    Load(usize),
    Shutdown,
}

/// Async layer prefetcher that replicates AirLLM's `ThreadPoolExecutor` pattern.
///
/// A dedicated background thread handles disk I/O so that the GPU/CPU can
/// compute the current layer while the next one is being loaded.
///
/// ```text
/// Main thread:          process layer N  ──────────────────> process layer N+1
/// Background thread:              load layer N+1 ──> ready  load layer N+2
/// ```
///
/// Usage pattern inside a forward pass:
/// ```ignore
/// prefetcher.prefetch(0); // kick off layer 0 load
/// for i in 0..n_layers {
///     let layer = prefetcher.wait(i)?;   // wait for layer i
///     prefetcher.prefetch(i + 1);        // start loading layer i+1
///     // … compute with `layer` …
///     // `layer` drops here, freeing RAM
/// }
/// ```
pub struct LayerPrefetcher {
    request_tx: mpsc::SyncSender<PrefetchRequest>,
    result_rx: mpsc::Receiver<(usize, Result<LayerWeights>)>,
}

impl LayerPrefetcher {
    /// Spawn the background loader thread.
    ///
    /// The loader is wrapped in `Arc` so it can be shared between this struct
    /// (which needs it for sync fallback) and the background thread.
    pub fn new(loader: Arc<LayerLoader>) -> Self {
        // Bounded channel: only buffer 1 pending request so the background
        // thread never races too far ahead.
        let (request_tx, request_rx) = mpsc::sync_channel::<PrefetchRequest>(1);
        let (result_tx, result_rx) =
            mpsc::channel::<(usize, Result<LayerWeights>)>();

        thread::spawn(move || {
            debug!("LayerPrefetcher background thread started");
            loop {
                match request_rx.recv() {
                    Ok(PrefetchRequest::Load(layer_idx)) => {
                        debug!("📥 Prefetcher: loading layer {}", layer_idx);
                        let result = loader.load_layer(layer_idx);
                        if result_tx.send((layer_idx, result)).is_err() {
                            debug!("LayerPrefetcher: result channel closed, shutting down");
                            break;
                        }
                    }
                    Ok(PrefetchRequest::Shutdown) | Err(_) => {
                        debug!("LayerPrefetcher background thread shutting down");
                        break;
                    }
                }
            }
        });

        Self {
            request_tx,
            result_rx,
        }
    }

    /// Request the background thread to start loading `layer_idx` from disk.
    ///
    /// Non-blocking: if the request channel is full, the request is silently
    /// dropped (the caller must fall back to `load_layer_sync`).
    pub fn prefetch(&self, layer_idx: usize) {
        match self.request_tx.try_send(PrefetchRequest::Load(layer_idx)) {
            Ok(_) => debug!("Prefetch requested: layer {}", layer_idx),
            Err(mpsc::TrySendError::Full(_)) => {
                debug!("Prefetch channel full, layer {} will be loaded on demand", layer_idx);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                warn!("Prefetcher background thread disconnected");
            }
        }
    }

    /// Block until the prefetched `layer_idx` is available.
    ///
    /// If the background thread already finished loading it, this returns
    /// immediately.  Otherwise it blocks until loading completes.
    ///
    /// Panics (via `anyhow::bail!`) if the received layer index does not match
    /// `layer_idx` — which would indicate a programming error in the caller.
    pub fn wait(&self, layer_idx: usize) -> Result<LayerWeights> {
        match self.result_rx.recv() {
            Ok((idx, result)) => {
                if idx != layer_idx {
                    anyhow::bail!(
                        "LayerPrefetcher: expected layer {}, received {}",
                        layer_idx,
                        idx
                    );
                }
                result
            }
            Err(e) => anyhow::bail!("LayerPrefetcher result channel closed: {}", e),
        }
    }

    /// Synchronous load without prefetching — used when `prefetch()` was not
    /// called in advance (e.g. the first layer in a sequence).
    pub fn load_layer_sync(&self, layer_idx: usize) -> Result<LayerWeights> {
        // Send a blocking request and wait for the result.
        self.request_tx
            .send(PrefetchRequest::Load(layer_idx))
            .map_err(|e| anyhow::anyhow!("LayerPrefetcher send error: {}", e))?;
        self.wait(layer_idx)
    }
}

impl Drop for LayerPrefetcher {
    fn drop(&mut self) {
        let _ = self.request_tx.send(PrefetchRequest::Shutdown);
    }
}
