# Iota gRPC API

## Run the gRPC server and test it
- Run the gRPC server: `GRPC_API_ADDRESS=127.0.0.1:50051 cargo run -p iota-grpc-api`
- Test the gPRC Server (try 1. w/o any indexes 2. only startIndex 3. only endIndex, and 4. startIndex and endIndex)
```bash
grpcurl -plaintext \                                       
  -import-path iota-grpc-api/proto \
  -proto checkpoint.proto \
  -d '{"startIndex": 3, "endIndex": 7}' \
  127.0.0.1:50051 iota.grpc.CheckpointService/StreamCheckpoints
```