# Telemetry library

#### Telemetry

Telemetry is a process for automatic data collection about the system's performance.
Its three core elements are:

- logs
- metrics
- traces

Telemetry is an expansion of monitoring, because in monitoring we collect data to understand the current
state of the system or detect failures, while telemetry is about collecting data to understand the behavior of the
system over time and allows for better troubleshooting issues.

The most commonly used form of telemetry is logging.
The **logs** reflect events happening in the system, however their context is static -
they provide information about the file, function or event. In the case of node software, many processes
happen concurrently and the same parts of code can be called in different situations,
so the static context is not enough to understand the correct flow of events or track
the source of a problem. That's where **tracing** comes into play.
Tracing is built around **spans** which, unlike logs representing a moment in time,
have a beginning and an end - they represent a period of time.

#### Spans

A span is a single unit of work and represents a specific operation, like a database query, and has a start and end
time. Spans can be linked together to show the flow of operations in a system. Each span can include:

- A name describing the operation
- Timing information (start and end)
- Attributes (key-value pairs) for extra context or relationships to other spans (parent-child).

Spans help understand the flow of requests through a system better.
The span filter, created by the `init` function ensures that only relevant spans are processed, which helps manage
performance and logging noise.

## Telemetry Subscribers

The `telemetry-subscribers` library expands its functionality by integrating with [Tokio tracing](https://github.com/tokio-rs/tracing) and `OpenTelemetry`. Tokio tracing is a separate library specifically designed for asynchronous Rust applications. And `OpenTelemetry` is an observability library with three main components:

- **Instrumentation** - library that provides subscribers for Tokio tracing to collect telemetry data: logs, metrics, and traces.
- **SDKs** - library that provides a set of tools to create custom telemetry data collectors.
- **Exporters** - export telemetry data to external systems like Prometheus, Jaeger, etc.

The `telemetry-subscribers` package includes many common subscribers, such as writing trace data to Jaeger, distributed tracing,
common logs and metrics destinations, etc. into an easy to configure common package. There are also
some unique layers such as one to automatically create Prometheus latency histograms for spans.

> We also purposely separate out logging levels from span creation. This is often needed by production apps
> as normally it is not desired to log at very high levels, but still desirable to gather sampled span data
> all the way down to TRACE level spans.

### Getting Started

Getting started is easy. In your app, add the following:

```rust
let config = telemetry::TelemetryConfig::new("my_app");
let guard = telemetry::init(config);
```

> It is important to retain the guard until the end of the program. Assign it in the main fn and keep it,
> for once it drops then the log output will stop.

There is a builder API available: just do `TelemetryConfig::new()...`. Another convenient initialization method
is `TelemetryConfig::new().with_env()`, that will populate the config from environment variables.

You can also run the example and see output in ANSI color:

```bash
cargo run --example easy-init
```

### Span Creation

To create a span we can use the general `span!` marco, adding span level:

```rust
let span = span!("handle_transaction", Level::TRACE);
```

Or use convenience versions, like: `trace_span!`, `error_span!`, `debug_span!` etc.

```rust
let span = trace_span!("handle_transaction");
```

### Span Instrumentation

After we create a span, we can use it to instrument a synchronous block of code with `.enter()` that returns a guard that exits the span when dropped.

```rust
let span = trace_span!("handle_transaction");  
let _enter = span.enter();
```

To attach a span to an asynchronous block of code, any object that implements Future, we use instrument:

```rust
async move {
...
}
.instrument(error_span!("jwk_updater_task"))

tokio::spawn(my_func.start().instrument(span));
```

Or, by using rust attribute instrument provided by tracing crate, here we skip saving function attributes:

```rust
#[instrument(name="span_name" level = "error", skip_all)]
```

## Usage and Implementation

The library is currently used in different `main` functions of the node, including tests and is typically initialized at the
beginning of a `main` function like:

```rust
// initialize tracing
let _guard = telemetry_subscribers::TelemetryConfig::new()
    .with_env()
    .init();
```

The following steps are taken during initialization:

1. The initialization phase begins by setting up the span filter, which determines which spans are recorded based on their level.
2. The configuration options are read from the environment variables or set programmatically.
3. After the span filter is set up, a collection of layers is initialized. These layers send data to `tokio-console` for debugging or integrate with Prometheus for measuring span latencies.
   Each layer will be enabled or disabled based on the configuration.
4. If OTLP tracing is enabled, an `OpenTelemetryLayer` will be set up for tracing **to either a file or an OTLP endpoint** based on environment settings.
5. After setting up all layers, a tracing subscriber is created with the configured layers and set as the global default. Ultimately, the function creates and returns `TelemetryGuards` and `TracingHandle` structs, which manage the tracing subscriber. They are active in the main function to ensure logging and tracing throughout the application's lifecycle.

## Library Features

The following features are available:

- `otlp` - this feature is enabled by default as it enables otlp tracing
- `json` - Bunyan formatter - JSON log output, optional
- `tokio-console` - [Tokio-console](https://github.com/tokio-rs/console) subscriber, optional
- `span latency metrics` - Prometheus metrics for span latencies
- `panic-hook` - custom panic hook, optional

## Configuration

To manage the configuration of the telemetry library, the `TelemetryConfig` struct is used.
The `TelemetryConfig` allows setting various configuration options, such as tracing with the OpenTelemetry protocol (OTLP), outputting JSON logs,
writing logs to files, setting log levels, defining span levels, setting panic hooks, or specifying Prometheus
registries.

```rust
/// Configuration for different logging/tracing options
#[derive(Default, Clone, Debug)]
pub struct TelemetryConfig {
    /// Enables export of tracing span data via OTLP. Can be viewed with grafana/tempo.
    /// Enabled if `TRACE_FILTER` env var is provided.
    pub enable_otlp_tracing: bool,
    /// Enables Tokio Console debugging on port 6669.
    /// Enabled if `TOKIO_CONSOLE` env var is provided.
    pub tokio_console: bool,
    /// Output JSON logs to stdout only.
    /// Enabled if `RUST_LOG_JSON` env var is provided.
    pub json_log_output: bool,
    /// If defined, write output to a file starting with this name, ex app.log.
    /// Provided by `RUST_LOG_FILE` env var.
    pub log_file: Option<String>,
    /// Log level to set ("error/warn/info/debug/trace"), defaults to "info".
    /// Provided by `RUST_LOG` env var.
    pub log_string: Option<String>,
    /// Span level - what level of spans should be created.  Note this is not
    /// same as logging level If set to None, then defaults to "info".
    /// Provided by `TOKIO_SPAN_LEVEL` env var.
    pub span_level: Option<Level>,
    /// Set a panic hook.
    pub panic_hook: bool,
    /// Crash on panic.
    /// Enabled if `CRASH_ON_PANIC` env var is provided.
    pub crash_on_panic: bool,
    /// Optional Prometheus registry - if present, all enabled span latencies
    /// are measured.
    pub prom_registry: Option<prometheus::Registry>,
    /// Sample rate for tracing spans, that will be used in the "TraceIdRatioBased" sampler.
    /// Provided by `SAMPLE_RATE` env var.
    pub sample_rate: f64,
    /// Add directive to include trace logs with provided target.
    pub trace_target: Option<Vec<String>>,
}
```

As for the node configuration, some of those fields are set in code, with option to configure from the outside,
but others can be updated. `TelemetryConfig` implements a `with_env` function in order to set the config fields
based on environment variables. After a `TelemetryConfig` instance is created and values are set, the `init`
function will enable it.

For that, the `init` first sets up a `EnvFilter` to manage which log messages are shown, based on the log level.
Per default, the log level is set to `info`, but it can be adjusted by setting the `log_string` variable.
Then, another filter is created for span levels.

## Metrics

Metrics are collected by Prometheus pull-based system, meaning that Prometheus scrapes metrics from the services it monitors. Metrics are stored in a time series database, which allows for powerful queries and visualization.
Prometheus can be used to monitor the health of the node, as well as to create alerts based on the metrics.

Metrics are served with a Prometheus scrape endpoint, by default at `<host>:9184/metrics`.

## Prometheus Layer

The `telemetry-subscriber` allows measuring the Tokio-tracing [span](https://docs.rs/tracing/latest/tracing/span/index.html) latencies that will be recorded into Prometheus histograms directly.
For that, a `prometheus::Registry` must be passed to the `TelemetryConfig` using the `with_prom_registry` function.
The name of the Prometheus histogram is `tracing_span_latencies(_sum/count/bucket)`.

```rust
let registry_service = iota_metrics::start_prometheus_server(metrics_address);
let prometheus_registry = registry_service.default_registry();

// Initialize logging
let (_guard, filter_handle) = telemetry_subscribers::TelemetryConfig::new()
    .with_env()
    .with_prom_registry(&prometheus_registry)
    .init();
```

In order to set up the subscriber, it enters the metrics runtime first and creates a `RegistryService` from
the [`iota-metrics` crate](iota-metrics.mdx).
It initializes the default Prometheus registry in the `RegistryService` and returns it.
The `metrics_address` is the address where the Prometheus metrics will be exposed and can be specified in the node's
configuration file.
Per default, it is set to `0.0.0.0:9400`.
After the default registry is created, it is passed to the `TelemetryConfig` with the `with_prom_registry` function.

### Implementation

Span latencies are configured currently for 15 buckets. This number could be changed to change granularity for the distribution to save space used in Prometheus.

```rust
// crates/telemetry-subscribers/src/lib.rs
if let Some(registry) = config.prom_registry {  
    let span_lat_layer = PrometheusSpanLatencyLayer::try_new(&registry, 15)  
        .expect("Could not initialize span latency layer");  
    layers.push(span_lat_layer.with_filter(span_filter.clone()).boxed());  
}
```

Latencies collected from spans are defined under the combination of the name `tracing_span_latencies_bucket` and the attribute `span_name`. Time values are saved in nanoseconds.
Only spans that were actually triggered are collected. Here is an example of histogram latency metric collected for `finalize_checkpoint` that indicates how many nanoseconds execution of this function took.

## Exporting Telemetry Data

The library provides a way to export telemetry data to external systems like Prometheus, Jaeger, etc.
The tracing architecture is based on the idea of [subscribers](https://github.com/tokio-rs/tracing#project-layout) which can be plugged into the tracing library to process and forward output to different sinks for viewing. Multiple subscribers can be active at the same time.
You can feed JSON logs, for example, through a local sidecar log forwarder such as [Vector](https://vector.dev), and then onwards to destinations such as ElasticSearch.
The use of a log and metrics aggregator such as Vector allows for easy reconfiguration without interrupting the validator server, as well as offloading observability traffic.

## Usage and further reading

To learn more about observability refer to [observability.md](./observability.md) file.

More detailed guidance about how to use the library is provided in different document. Here is the list of available guides:

- [Configuration](./observability_guides.md) - configuration and list of all environment variables.
- [Logs and std output](./observability_guides.md)
- How to enable tracing and span output to a
  - [file](./observability_guides.md)
  - [JSON formatted file](./observability_guides.md) - can be used, e.g. with ElasticSearch, and
  - [OTLP endpoint](./observability_guides.md) - sending trace data to an OTLP endpoint that can be read by Grafana Tempo or Jaeger
- [Tokio console](./observability_guides.md/#live-async-inspection-with-tokio-console) - how to enable Tokio console for debugging Rust apps using Tokio in real time
- [Custom panic hook](./observability_guides.md/#custom-panic-hook) - how to set a custom panic hook
