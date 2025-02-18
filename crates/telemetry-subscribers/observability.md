# Logging, Tracing, Metrics, and Observability

Good observability capabilities are key to the development and growth of IOTA. This is made more challenging by the distributed and asynchronous nature of IOTA, with multiple client and validator processes distributed over a potentially global network.

The observability stack in IOTA is mainly based on the [Tokio tracing](https://tokio.rs/blog/2019-08-tracing) library and implemented as `telemetry-subscribers` (for more information about the library see [README](README.md). The rest of this document highlights specific aspects of achieving good observability through structured logging and metrics in IOTA.

>**info**
>
>The output here is largely for the consumption of IOTA operators, administrators, and developers. The content of logs and traces do not represent the authoritative, certified output of validators and are subject to potentially byzantine behavior.

## Contexts, scopes, and tracing transaction flow

In a distributed and asynchronous system like IOTA, one cannot rely on looking at individual logs over time in a single thread. To solve this problem, we use the approach of **structured logging**. Structured logging offers a way to tie together logs, events, and blocks of functionality across threads and process boundaries.

### Spans and events

In the [Tokio tracing](https://tokio.rs/blog/2019-08-tracing) library, structured logging is implemented using [spans and events](https://docs.rs/tracing/0.1.31/tracing/index.html#core-concepts).
Spans cover a whole block of functionality - like one function call, a future or asynchronous task, etc. They can be nested, and **key-value** pairs in spans give context to **events** or **logs** inside the function.

- **spans** and **key-value** pairs - represent a block of functionality (e.g., a function call) and can contain key-value pairs that provide context to enclosed logs (e.g, a transaction ID).
- **spans**  -  track time spent in different sections of code, enabling distributed tracing functionality.
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

Spans capture not a single event but an entire block of time; so start, end, duration, etc. can be captured and analyzed for tracing, performance analysis, and so on.

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

## Logging levels

This is always tricky, to balance the right amount of verbosity especially by default -- while keeping in mind this is a high performance system.

| Level | Type of Messages                                                                                           |
| ----- | ---------------------------------------------------------------------------------------------------------- |
| Error | Process-level faults (not transaction-level errors, there could be a ton of those)                         |
| Warn  | Unusual or byzantine activity                                                                              |
| Info  | High level aggregate stats, major events related to data sync, epoch changes.                              |
| Debug | High level tracing for individual transactions, eg Gateway/client side -> validator -> Move execution etc. |
| Trace | Extremely detailed tracing for individual transactions                                                     |
|       |                                                                                                            |

Going from info to debug results in a much larger spew of messages.

Use the `RUST_LOG` environment variable to set both the overall logging level as well as the level for individual components. 

Filtering down to specific spans or tags within spans is possible with `TRACE_FILTER`.
For more details, see the [EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) topic.



## Configuration

All the span and tracing parameters:

| Related Feature                                            | Corresponding `TelemetryConfig`             | Environment Variable                         | Values                                                                                                                                                                                                                     |
|------------------------------------------------------------|---------------------------------------------|----------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| [Logging levels](#logs-and-std-output)                     | `log_file`                                  | `RUST_LOG_FILE`                              | Set filepath to save logs.                                                                                                                                                                                                 |
|                                                            | `enable_otlp_tracing`                       | `TRACE_FILTER`                               | Value could be defined with `LevelFilter` in `tracing_subscriber::filter` - Rust or specified directly for selected module `TRACE_FILTER="my_crate::module=info"`. By default, it sets the trace level based on `RUST_LOG. |
|                                                            | -                                           | `TRACE_FILE`                                 | `path/to/file` - save trace data to txt file, instead of sending via OTLP protocol.                                                                                                                                        |
|                                                            | -                                           | `OTLP_ENDPOINT`                              | `Opentelemetry` by default sends trace data with `OpenTelemetry` protocol default endpoint `http://localhost:4317`.                                                                                                        |
|                                                            | -                                           | `OTEL_SERVICE_NAME`                          | Service name for OTLP, default `iota-node`.                                                                                                                                                                                |
|                                                            | `sample_rate`                               | `SAMPLE_RATE`                                | Values `rate>=1` - always sample, `rate<0` never sample, `rate<1` - sample rate with `rate` probability, e.g. for `0.5` there is 50% chance that trace will be sampled.                                                    |
| [Tracing output to JSON formatted file](#file)             | `json_log_output`                           | `RUST_LOG_JSON`<br/>  `TRACE_FILE`           | `ok`  <br/>          `path/to/file` - save trace data to file in JSON format.                                                                                                                                              |
| [Custom panic hook](#custom-panic-hook)                    | `crash_on_panic`                            | `CRASH_ON_PANIC`                             | `ok` - crash on panic.                                                                                                                                                                                                     |
| [Tokio console](#live-async-inspection-with-tokio-console) | `tokio_console`<br/">    `tokio_span_level` | `TOKIO_CONSOLE` <br/>     `TOKIO_SPAN_LEVEL` | `ok` - enable Tokio console.     <br/>`trace`, `debug`, `info`, `warn`, `error` - set the span level.                                                                                                                      |



## Viewing logs, traces, metrics


## Logs and std output (default)

By default, logs (but not spans) are formatted for human readability and output to stdout, with key-value tags at the end of every line.
See the configuration guide: [Logging levels](#logs-and-std-output) and [Logs and std output](observability_guides.md#logs-and-std-output).


### Tracing and span output

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


### Jaeger (seeing distributed traces)
Jaeger is one way to visualize tracing data. It is an open-source, end-to-end distributed tracing tool. It can n visualize the traces collected by the tracing crate.

To try in practice, follow this guide: [Jaeger](observability_guides.md#jaeger).

### Automatic Prometheus span latencies

Included in this library is a tracing-subscriber layer named `PrometheusSpanLatencyLayer`. It will create
a Prometheus histogram to track latencies for every span in your app, which is super convenient for tracking
span performance in production apps.

Enabling this layer is done programmatically, by passing in a Prometheus registry to `TelemetryConfig`. 

In the node it is enabled [here](https://github.com/iotaledger/iota/blob/cc3e84892b0e1f133905aa1a146a7016231af5f4/crates/iota-node/src/main.rs#L77).

For more information on metrics and latency histograms created by this layer, see the [Latencies](../iota-metrics/README.md) guide.

### Live async inspection / Tokio Console

[Tokio-console](https://github.com/tokio-rs/console) is an awesome CLI tool designed to analyze and help debug Rust apps using Tokio, in real time! It relies on a special subscriber.

See how to use Tokio console in this guide: [Live async inspection with Tokio Console](observability_guides.md#live-async-inspection-with-tokio-console).


### Memory profiling
Memory profiling is might be useful to analyze the memory usage of an application, helping to identify memory leaks and optimize memory consumption. 
IOTA uses the [jemalloc](https://jemalloc.net/)  memory allocator by default, which includes a lightweight sampling profiler suitable for production use.  

For detailed instructions on setting up and using memory profiling in IOTA, refer to the Memory Profiling Guide.
