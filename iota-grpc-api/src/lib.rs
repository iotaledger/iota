use std::{pin::Pin, sync::Arc};

use futures::Stream;
use tonic::{Request, Response, Status};

pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

use checkpoint::{Checkpoint, StreamRequest, checkpoint_service_server::CheckpointService};

#[derive(Clone, Debug, Default)]
pub struct GrpcApiConfig {
    pub grpc_api_address: Option<String>,
}

#[derive(Debug, Default)]
pub struct CheckpointGrpcService {
    pub checkpoints: Arc<Vec<Checkpoint>>,
}

type CheckpointStream = Pin<Box<dyn Stream<Item = Result<Checkpoint, Status>> + Send>>;

#[tonic::async_trait]
impl CheckpointService for CheckpointGrpcService {
    type StreamCheckpointsStream = CheckpointStream;

    async fn stream_checkpoints(
        &self,
        request: Request<StreamRequest>,
    ) -> Result<Response<Self::StreamCheckpointsStream>, Status> {
        let req = request.into_inner();
        let latest = self.checkpoints.last().map(|c| c.index).unwrap_or(0);
        let (start, end) = match (req.start_index, req.end_index) {
            (Some(start), Some(end)) => (start, std::cmp::min(latest, end)),
            (Some(start), None) => (start, latest),
            (None, Some(end)) => (end, end),
            (None, None) => (0, latest),
        };
        let filtered: Vec<_> = self
            .checkpoints
            .iter()
            .filter(|cp| cp.index >= start && cp.index <= end)
            .cloned()
            .collect();
        let stream = futures::stream::iter(filtered.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream) as CheckpointStream))
    }
}
