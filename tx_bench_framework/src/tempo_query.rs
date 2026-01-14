// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn build_tempo_digest_regex(digests: &[String]) -> String {
    digests
        .iter()
        .map(|d| format!(r#"TransactionDigest\\({}\\)"#, d))
        .collect::<Vec<_>>()
        .join("|")
}

pub fn print_tempo_traceql_queries(
    tempo_service_name: &str,
    span_name: &str,
    sender: &str,
    digests: &[String],
) {
    let digest_re = build_tempo_digest_regex(digests);

    println!("\n--- Tempo / TraceQL Metrics (Explore → Metrics mode) ---");

    println!(
        r#"{{
  resource.service.name = "{svc}" &&
  span:name = "{span}" &&
  span.sender = "{sender}" &&
  span.tx_digest =~ "{re}"
}} | avg_over_time(span:duration) | quantile_over_time(span:duration, .99, .95, .50)"#,
        svc = tempo_service_name,
        span = span_name,
        sender = sender,
        re = digest_re
    );
}
