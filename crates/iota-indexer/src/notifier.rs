use tokio::sync::broadcast;

use crate::types::IndexedCheckpoint;

pub type CheckpointNotifier = broadcast::Sender<IndexedCheckpoint>;
