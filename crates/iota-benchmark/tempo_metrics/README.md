## Connect to the Tempo

### Local run

Run from iota-benchmark/tempo_metrics:

```bash
docker-compose up -d
```

Export following variable before start the benchmark:

```bash
export OTEL_EXPORTER_OTLP_PROTOCOL=grpc                                            
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317
export OTEL_TRACES_SAMPLER=always_on
export OTLP_ENDPOINT=http://127.0.0.1:4317  
export TRACE_FILTER=[handle_transaction]=trace,[process_certificate]=trace
```
