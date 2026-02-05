# Observability Guides

`telemetry-subscribers` provides a way to export telemetry data to external systems like Prometheus, Jaeger, etc.
The tracing architecture is based on the idea of [subscribers](https://github.com/tokio-rs/tracing#project-layout) which can be plugged into the tracing library to process and forward output to different sinks for viewing. Multiple subscribers can be active at the same time.
You can feed JSON logs, for example, through a local sidecar log forwarder such as [Vector](https://vector.dev), and then onwards to destinations such as ElasticSearch.
The use of a log and metrics aggregator such as Vector allows for easy reconfiguration without interrupting the validator server, as well as offloading observability traffic.

## Logs and `std` Output

By default, logs (but not spans) are formatted for human readability and output to `stdout`, with key-value tags at the end of every line.
If `log_file` is specified, log outputs are written to a daily-rotated file.
You can set `log_file` to configure custom logging output and filtering by:

To save logs to file - set the `RUST_LOG_FILE` environment variable.
To save logs in JSON format - set the `RUST_LOG_JSON` environment variable.
By default, logs are printed to stdout.

**NOTE: JSON output requires the `json` crate feature to be enabled.**

## Tracing and Span Output

`telemetry-subscribers` can output trace data to a file or an OTLP endpoint. It supports only a single at a time.

### Export Traces to a File

To save traces to a file:

1. Set the `TRACE_FILE` environment variable to the path of the file.

> **Note**: If `TRACE_FILE` is not set `telemetry-subscribers` will send the trace data to the OTLP endpoint.

### Starting `opentelemetry` Tracing

1. Make sure to start a node with the admin interface enabled. By default, it is enabled but restricted to `localhost`.

> **tip**
> when running a node with docker you might need to change default 127.0.0.1 binding to `0.0.0.0` to access the admin console.

2. (optional) Provide path to file where traces should be saved
   with `TRACE_FILE` environment variable instead of sending it via OTLP.
3. Set `TRACE_FILTER=off` before starting a node or local network, e.g.:

**For a node run from source:**

```shell
TRACE_FILTER=off ./target/release/iota-node --config-path /o  
pt/iota/config/fullnode-template.yaml
```

**For a node started with docker**, add environment variable to docker compose, e.g.:

```yaml
image: iotaledger/iota-node:testnet
environment:
  - TRACE_FILTER=off
```

4. Enable tracing via admin console, so that spans are sent either with OTLP or saved to a file.

Use following command to start tracing for a specific time duration:

```shell
curl -X POST 'http://127.0.0.1:1337/enable-tracing?filter=iota-node=trace,info&duration=20s
```

#### Explore spans via Grafana and Tempo

It is possible to freely explore spans sent by telemetry traces through the Grafana dashboard configured with Tempo instance.

1. Run Grafana and Tempo. We do not provide setup for a standalone node, but ready environment for local network is in docker/grafana-local.

2. Start node with OTLP protocol running (see previous section [Starting opentelemetry tracing](#starting-opentelemetry-tracing)).

3. Go to Grafana → Data Explorer → data source: Tempo → Run query {}

### Jaeger

To see nested spans visualized with [Jaeger](https://www.jaegertracing.io), do the following:

1. Run this to get a local Jaeger container:

   ```bash
   docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest
   ```

2. Run IOTA like this (trace enables the most detailed spans):

   ```bash
   TRACE_FILTER=1 RUST_LOG="info,iota_core=trace" ./iota start
   ```

3. Run some transfers with IOTA CLI client, or run the benchmarking tool.
4. Browse to `http://localhost:16686/` and select IOTA as the service.

> **info**
>
> Separate spans (that are not nested) are not connected as a single trace for now.

#### How to check latency of a selected function

Enabling the tracing steps from [Starting `opentelemetry` tracing](#starting-opentelemetry-tracing) are not needed in
this case,
as `PrometheusSpanLatencyLayer` is enabled independently of tracing with OLTP. However, note that default span level
is set to `info`, so only spans with level `error` and `info` will be sent as metrics.

1. Create and enable span for chosen filter level, see [Span creation](README.md#span-creation).

   As an example, we create a span for a function that can be triggered with the admin console. `skip_all` omits passing function attributes.

   ```rust
   #[instrument(level="info", skip_all)]
   async fn node_config(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
   ```

2. Build a node or a docker image covering any code additions.

3. Make sure that the function/span you want to measure is executed within that timeframe, and that the filter level is
   covering the level defined in the instrument attribute.

   You can run one of the admin endpoints to make sure that span will be created:

   ```rust
   curl -X GET 'http://127.0.0.1:1337/capabilities'
   ```

4. Afterwards, metrics should be available in node’s metrics list, under its span_name.

   ```bash
   curl -X GET 'http://127.0.0.1:9184/metrics' | grep tracing_span_latencies_bucket
   ```

   The output should look like this:

   ```shell
   tracing_span_latencies_bucket{span_name="capabilities",le="24024296.060942296"} 0                                                                                                                
   tracing_span_latencies_bucket{span_name="capabilities",le="92440174.0609853"} 0                                                                                                                  
   tracing_span_latencies_bucket{span_name="capabilities",le="355689330.4490061"} 0                                                                                                                 
   tracing_span_latencies_bucket{span_name="capabilities",le="1368613820.5646057"} 0                                                                                                                
   tracing_span_latencies_bucket{span_name="capabilities",le="5266123072.839788"} 4                                                                                                                 
   tracing_span_latencies_bucket{span_name="capabilities",le="20262876058.678875"} 4                                                                                                                
   tracing_span_latencies_bucket{span_name="capabilities",le="77967062389.21445"} 4                                                                                                                 
   tracing_span_latencies_bucket{span_name="capabilities",le="+Inf"} 4
   ```

### Live Async Inspection With Tokio Console

[Tokio-console](https://github.com/tokio-rs/console) is a CLI tool designed to analyze and help debug Rust apps using Tokio, in real time! It relies on a special subscriber.

#### On the node side

1. Build node with
   - a special rust flag (`tokio_unstable` config)
   - `tokio-console` feature enabled by adding `--features` to your cargo command.
2. Run node with `TOKIO_CONSOLE=1` environment variable to enable the console.

The whole command:

```shell
TOKIO_CONSOLE=1 RUSTFLAGS="--cfg tokio_unstable" cargo run --bin iota-node --features tokio-console -- --config-path fullnode.yaml
```

> **tip**
>
> Adding Tokio-console support might significantly slow down IOTA validators/gateways.

#### On the tokio console side

1. Clone the [console](https://github.com/tokio-rs/console) repo.

2. Run the console:

```shell
cargo run
```

> **tip**
>
> In case of problems with the console not showing any output, try installing it via cargo `cargo install --locked tokio-console`.

**NOTE**: It is possible to set the Tokio TRACE logs with `TOKIO_SPAN_LEVEL` however it is NOT necessary.
It says that in the docs, but there's no need to change Tokio logging levels at all.
The console subscriber has a special filter enabled taking care of that.

By default, the Tokio console listens on port 6669. To change this setting as well as other setting such as
the retention policy, please see the [configuration](https://docs.rs/console-subscriber/latest/console_subscriber/struct.Builder.html#configuration) guide.

### Custom Panic Hook

This library installs a custom panic hook which records a log (event) at ERROR level using the tracing
crate. This allows span information from the panic to be properly recorded as well.
It is connected to two `TelemetryConfig` settings: `panic_hook` and `crash_on_panic`.

- `panic_hook` is enabled by default in the `TelemetryConfig` struct, but can be disabled in the code if desired.
- `crash_on_panic` is disabled by default, but can be enabled with `CRASH_ON_PANIC`.

To exit the process on panic, set the `CRASH_ON_PANIC` environment variable when starting a node.

### Memory Profiling

IOTA Docker images include [heaptrack](https://github.com/KDE/heaptrack), a heap memory profiler that tracks all memory allocations and can help identify memory leaks and excessive memory usage patterns. Heaptrack provides detailed information about where memory is allocated and can generate comprehensive reports for analysis.

For optimal profiling results with readable stack traces and function names, build Docker images with debug symbols by setting RUSTFLAGS before building:

```bash
export RUSTFLAGS="-C debuginfo=2"
# Then build your Docker image
docker build --build-arg RUSTFLAGS="-C debuginfo=2" -t iotaledger/iota-node .
```

Or use the provided debug build script:

```bash
./docker/iota-node/build_debuginfo.sh
```

To profile memory usage with Docker containers, you can run the container with heaptrack enabled:

```bash
docker run -it --rm \
  -v $(pwd)/heaptrack-data:/tmp/heaptrack-data \
  iotaledger/iota-node \
  heaptrack -o /tmp/heaptrack-data/iota-profile iota-node [your-node-arguments]
```

This will:

1. Run the IOTA node under heaptrack profiling
2. Save the profiling data to a volume-mounted directory for analysis
3. Generate a profile file named `iota-profile.heaptrack` in the mounted directory

To analyze the generated profiles:

1. Install heaptrack on your host system. On Debian/Ubuntu: `apt-get install heaptrack-gui heaptrack`.
2. Open the profile file with the heaptrack GUI: `heaptrack_gui /path/to/iota-profile.heaptrack`.
3. Or generate text reports: `heaptrack_print /path/to/iota-profile.heaptrack`.

> **tip**
>
> For production environments, consider the performance impact of memory profiling. Heaptrack can introduce overhead, so use it primarily for debugging memory issues or performance analysis in non-production environments.
>
> You can also attach heaptrack to an already running process inside a Docker container:
>
> ```bash
> docker exec -it <container-name> heaptrack -p <pid> -o /tmp/profile-name
> ```
