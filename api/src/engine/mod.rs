// Facade del motor de inferencia.
// Todo el código vive en inference.rs; este módulo lo re-expone
// con la ruta canónica `crate::engine::*`.
pub use crate::inference::{
    CompletionRequest, InferenceActor, StreamingStatus,
};
