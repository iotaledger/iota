use std::sync::Arc;

use iota_grpc_api::{
    CheckpointGrpcService,
    checkpoint::{Checkpoint, StreamRequest, checkpoint_service_server::CheckpointService},
};
use tokio_stream::StreamExt;
use tonic::Request;

fn test_service() -> CheckpointGrpcService {
    let checkpoints = (0..=10)
        .map(|i| Checkpoint {
            index: i,
            data: format!("cp{i}"),
        })
        .collect();
    CheckpointGrpcService {
        checkpoints: Arc::new(checkpoints),
    }
}

#[tokio::test]
async fn test_start_index_only() {
    let svc = test_service();
    let req = StreamRequest {
        start_index: Some(5),
        end_index: None,
    };
    let stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let result: Vec<_> = stream.map(|c| c.unwrap().index).collect().await;
    assert_eq!(result, (5..=10).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_start_and_end_index() {
    let svc = test_service();
    let req = StreamRequest {
        start_index: Some(3),
        end_index: Some(7),
    };
    let stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let result: Vec<_> = stream.map(|c| c.unwrap().index).collect().await;
    assert_eq!(result, (3..=7).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_end_index_only() {
    let svc = test_service();
    let req = StreamRequest {
        start_index: None,
        end_index: Some(4),
    };
    let stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let result: Vec<_> = stream.map(|c| c.unwrap().index).collect().await;
    assert_eq!(result, vec![4]);
}
