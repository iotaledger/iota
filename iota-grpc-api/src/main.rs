use std::sync::Arc;

use iota_grpc_api::{
    CheckpointGrpcService,
    checkpoint::{Checkpoint, checkpoint_service_server::CheckpointServiceServer},
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For demo: get address from env or use default
    let grpc_api_address = std::env::var("GRPC_API_ADDRESS").ok();
    if let Some(addr) = grpc_api_address {
        let addr = addr.parse()?;
        // Mock checkpoints for now
        let checkpoints = (0..=10)
            .map(|i| Checkpoint {
                index: i,
                data: format!("cp{i}"),
            })
            .collect();
        let service = CheckpointGrpcService {
            checkpoints: Arc::new(checkpoints),
        };
        println!("Starting gRPC server on {addr}");
        Server::builder()
            .add_service(CheckpointServiceServer::new(service))
            .serve(addr)
            .await?;
    } else {
        println!("GRPC API not enabled (set GRPC_API_ADDRESS env var to enable)");
    }
    Ok(())
}
