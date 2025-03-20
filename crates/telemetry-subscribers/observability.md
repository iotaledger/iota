# Logging, Tracing, Metrics, and Observability

Good observability capabilities are key to the development and growth of IOTA. This is made more challenging by the distributed and asynchronous nature of IOTA, with multiple client and validator processes distributed over a potentially global network.

The observability stack in IOTA is mainly based on the [Tokio tracing](https://tokio.rs/blog/2019-08-tracing) library and implemented as `telemetry-subscribers` (for more information about the library see [README](README.md). The rest of this document highlights specific aspects of achieving good observability through structured logging and metrics in IOTA.

> **info**
>
> The output here is largely for the consumption of IOTA operators, administrators, and developers. The content of logs and traces do not represent the authoritative, certified output of validators and are subject to potentially byzantine behavior.

## Contexts, Scopes, and Tracing Transaction Flow

In a distributed and asynchronous system like IOTA, one cannot rely on looking at individual logs over time in a single thread. To solve this problem, we use the approach of **structured logging**. Structured logging offers a way to tie together logs, events, and blocks of functionality across threads and process boundaries.

### Spans and Events

In the [Tokio tracing](https://tokio.rs/blog/2019-08-tracing) library, structured logging is implemented using [spans and events](https://docs.rs/tracing/0.1.31/tracing/index.html#core-concepts).
Spans cover a whole block of functionality - like one function call, a future or asynchronous task, etc. They can be nested, and **key-value** pairs in spans give context to **events** or **logs** inside the function.

- **spans** and **key-value** pairs - represent a block of functionality (e.g., a function call) and can contain key-value pairs that provide context to enclosed logs (e.g, a transaction ID).
- **spans** - track time spent in different sections of code, enabling distributed tracing functionality.
- individual **logs** - can also add **key-value** pairs to aid in parsing, filtering and aggregation.

Below is an example of specific **key-value** pairs that are useful for tracing and logging in IOTA system:

- TX Digest
- Object references/ID, when applicable
- Address
- Certificate digest, if applicable
- For Client HTTP endpoint: route, method, status
- Epoch
- Host information, for both clients and validators

#### Key-value pairs schema

Spans capture not a single event, but an entire block of time; so start, end, duration, etc. can be captured and analyzed for tracing, performance analysis, and so on.

#### Tags - keys

The idea is that every event and span would get tagged with key-value pairs. Events that log within any context or nested contexts would also inherit the context-level tags.

These tags represent _fields_ that can be analyzed and filtered by. For example, one could filter out broadcasts and see the errors for all instances where the bad stake exceeded a certain amount, but not enough for an error.

In the digest

```rust
#[instrument(level = "trace", skip_all, fields(tx_digest =? effects.transaction_digest()), err)]
pub async fn process_tx(effects: &Effects) {
    // ...
    info!("Checked locks");
    // ...
}
```

`process_tx` is a span that covers handling the initial transaction request, and "Checked locks" is a single log message within the transaction handling method in the validator.

Every log message that occurs within the span inherits the key-value properties defined in the span, including the `tx_digest` and any other fields that are added. Log messages can set their own keys and values. The fact that logs inherit the span properties allows you to trace, for example, the flow of a transaction across thread and process boundaries.

## Logging Levels

Balancing the right amount of verbosity, especially by default, while keeping in mind this is a high performance system
is always tricky.

| Level | Type of Messages                                                                                             |
| ----- | ------------------------------------------------------------------------------------------------------------ |
| Error | Process-level faults (not transaction-level errors, there could be a ton of those)                           |
| Warn  | Unusual or Byzantine activity                                                                                |
| Info  | High level aggregate stats, major events related to data sync, epoch changes.                                |
| Debug | High level tracing for individual transactions, e.g. Gateway/client side -> validator -> Move execution etc. |
| Trace | Extremely detailed tracing for individual transactions                                                       |

Going from `info` to `debug` results in a much larger spew of messages.

Use the `RUST_LOG` environment variable to set both the overall logging level and the level for individual components.

Filtering down to specific spans or tags within spans is possible with `TRACE_FILTER`.
For more details, see the [`EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) topic.

## Configuration

All the span and tracing parameters:

| Corresponding `TelemetryConfig` | Environment Variable | Values                                                                                                                                                                                                                                                                                                          |
| ------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `enable_otlp_tracing`           | `TRACE_FILTER`       | Enables export of tracing span data via OTLP. Can be viewed with grafana/tempo. Value could be defined with `LevelFilter` in Rust via `tracing_subscriber::filter` - or specified directly for selected module `TRACE_FILTER="my_crate::module=info"`. By default, it sets the trace level based on `RUST_LOG`. |
| -                               | `OTLP_ENDPOINT`      | `Opentelemetry` by default sends trace data with `OpenTelemetry` protocol default endpoint `http://localhost:4317`.                                                                                                                                                                                             |
| -                               | `OTEL_SERVICE_NAME`  | Service name for OTLP, default `iota-node`.                                                                                                                                                                                                                                                                     |
| -                               | `TRACE_FILE`         | `path/to/file` - save trace data to txt file, instead of sending via OTLP protocol.                                                                                                                                                                                                                             |
| `tokio_console`                 | `TOKIO_CONSOLE`      | `ok` - Enables Tokio Console debugging on port 6669.                                                                                                                                                                                                                                                            |
| `json_log_output`               | `RUST_LOG_JSON`      | `ok` - Output logs in JSON format.                                                                                                                                                                                                                                                                              |
| `log_file`                      | `RUST_LOG_FILE`      | Set file path to save logs.                                                                                                                                                                                                                                                                                     |
| `log_string`                    | `RUST_LOG`           | Log level to set (`error/warn/info/debug/trace`), defaults to `info`.                                                                                                                                                                                                                                           |
| `span_level`                    | `TOKIO_SPAN_LEVEL`   | Set the level of spans that should be created (`error/warn/info/debug/trace`). Note this is not the same as logging level. If set to None, then defaults to `info`.                                                                                                                                             |
| `crash_on_panic`                | `CRASH_ON_PANIC`     | `ok` - crash on panic.                                                                                                                                                                                                                                                                                          |
| `sample_rate`                   | `SAMPLE_RATE`        | Sample rate for tracing spans. Values `rate>=1` - always sample, `rate<0` never sample, `rate<1` - sample rate with `rate` probability, e.g. for `0.5` there is 50% chance that trace will be sampled.                                                                                                          |

## Viewing Logs, Traces and Metrics

### Logs and `std` Output (default)

By default, logs (but not spans) are formatted for human readability and output to stdout, with key-value tags at the end of every line.

Detailed span start and end logs can be generated by defining the `json_log_output` config variable.
Note that this causes all output to be in JSON format, which is not as human-readable, so it is not enabled by default.
This output can easily be fed to backends such as ElasticSearch for indexing, alerts, aggregation, and analysis.

See the configuration guide: [Logs and std output](observability_guides.md#logs-and-std-output).
And log levels in section [Logging levels](#logging-levels).

### Tracing and Span Output

It is possible to generate detailed span start and end logs. This causes all output to be in JSON format, which is not as human-readable, so it is not enabled by default.

You can send this output to a tool or service for indexing, alerts, aggregation, and analysis.

The following example output shows _certificate_ processing in the authority with span logging. Note the `START` and `END` annotations, and notice how `DB_UPDATE_STATE` which is nested is embedded within `PROCESS_CERT`. Also notice `elapsed_milliseconds`, which logs the duration of each span.

```bash
{"v":0,"name":"iota","msg":"[PROCESS_CERT - START]","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.241421Z","target":"iota_core::authority_server","line":67,"file":"iota_core/src/authority_server.rs","tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493"}
{"v":0,"name":"iota","msg":"[PROCESS_CERT - EVENT] Read inputs for transaction from DB","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.246688Z","target":"iota_core::authority","line":393,"file":"iota_core/src/authority.rs","num_inputs":2,"tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493"}
{"v":0,"name":"iota","msg":"[PROCESS_CERT - EVENT] Finished execution of transaction with status Success { gas_used: 18 }","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.246759Z","target":"iota_core::authority","line":409,"file":"iota_core/src/authority.rs","gas_used":18,"tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493"}
{"v":0,"name":"iota","msg":"[DB_UPDATE_STATE - START]","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.247888Z","target":"iota_core::authority","line":430,"file":"iota_core/src/authority.rs","tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493"}
{"v":0,"name":"iota","msg":"[DB_UPDATE_STATE - END]","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.248114Z","target":"iota_core::authority","line":430,"file":"iota_core/src/authority.rs","tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493","elapsed_milliseconds":0}
{"v":0,"name":"iota","msg":"[PROCESS_CERT - END]","level":20,"hostname":"Evan-MLbook.lan","pid":51425,"time":"2022-03-08T22:48:11.248688Z","target":"iota_core::authority_server","line":67,"file":"iota_core/src/authority_server.rs","tx_digest":"t#d1385064287c2ad67e4019dd118d487a39ca91a40e0fd8e678dbc32e112a1493","elapsed_milliseconds":2}
```

Check following guides related to tracing and span output:

- [Enabling tracing](observability_guides.md#starting-opentelemetry-tracing)
- [Export traces to file](observability_guides.md#export-traces-to-file-and-json-format)
- [Explore spans with Grafana and Tempo](observability_guides.md#explore-spans-via-grafana-and-tempo).

### Jaeger (Seeing Distributed Traces)

Jaeger is one way to visualize tracing data. It is an open-source, end-to-end distributed tracing tool. It can visualize the traces collected by the tracing crate.

To try in practice, follow this guide: [Jaeger](observability_guides.md#jaeger).

### Automatic Prometheus Span Latencies

A tracing-subscriber layer named `PrometheusSpanLatencyLayer` is included in this library. It will create
a Prometheus histogram to track latencies for every span in your app, which is super convenient for tracking
span performance in production apps.

Enabling this layer is done programmatically, by passing in a Prometheus registry to `TelemetryConfig`.

In the node, it is enabled [here](https://github.com/iotaledger/iota/blob/cc3e84892b0e1f133905aa1a146a7016231af5f4/crates/iota-node/src/main.rs#L77).

Span latency are configured currently for 15 buckets. This number could be changed to adjust
granularity for the distribution to save space used in Prometheus

```rust
// crates/telemetry-subscribers/src/lib.rs
if let Some(registry) = config.prom_registry {
    let span_lat_layer = PrometheusSpanLatencyLayer::try_new(&registry, 15)
    .expect("Could not initialize span latency layer");
    layers.push(span_lat_layer.with_filter(span_filter.clone()).boxed());
}
```

Latencies collected from spans are defined under combination of the name `tracing_span_latencies_bucket` and the attribute `span_name`. Time values are saved in nanoseconds.
Only spans that were actually triggered are collected. Here is an example of histogram latency metric collected for
`finalize_checkpoint` that indicates how many nanoseconds execution of this function took. In this example, span life
corresponds to the execution time of `finalize_checkpoint`.

```rust
#[instrument(level = "info", skip_all, fields(seq = ?checkpoint.sequence_number(), epoch = ?epoch_store.epoch()))]
async fn finalize_checkpoint(...
```

To get all latency histograms created from spans you can use:

```shell
curl -X GET 'http://127.0.0.1:9184/metrics' | grep tracing_span_latencies_bucket
```

```shell
tracing_span_latencies_bucket{span_name="finalize_checkpoint",le="28483.952601417557"} 0
tracing_span_latencies_bucket{span_name="finalize_checkpoint",le="109599.94539447156"} 13
tracing_span_latencies_bucket{span_name="finalize_checkpoint",le="421716.3326508745"} 53946
```

Only spans that are actually created will be visible in that list. Furthermore, you can use Grafana dashboard to visualize histogram.

#### Explore span latencies through Prometheus metrics

As explained in [Automatic Prometheus span latencies](./observability.md#automatic-prometheus-span-latencies),
IOTA node implements `PrometheusSpanLatencyLayer`. A new tracing layer is created that attaches to span creation (`on_new_span`)
and span closure (`on_closeand`) creates prometheus histogram metric based on measured span lifetime.

Any created span will be shown under node metrics endpoint.

Check this [guide](observability_guides.md#how-to-check-latency-of-a-selected-function) on how to add new latency histograms.

### Live Async Inspection / Tokio Console

[Tokio-console](https://github.com/tokio-rs/console) is an awesome CLI tool designed to analyze and help debug Rust apps using Tokio, in real time! It relies on a special subscriber.

See how to use Tokio console in this guide: [Live async inspection with Tokio Console](observability_guides.md#live-async-inspection-with-tokio-console).

### Memory Profiling

Memory profiling is might be useful to analyze the memory usage of an application, helping to identify memory leaks and optimize memory consumption.
IOTA uses the [jemalloc](https://jemalloc.net/) memory allocator by default, which includes a lightweight sampling profiler suitable for production use.

For detailed instructions on setting up and using memory profiling in IOTA, refer to the Memory Profiling Guide.
