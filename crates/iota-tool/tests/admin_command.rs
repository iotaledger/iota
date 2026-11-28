// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Runs the `iota-tool` binary against the traffic control routes of a test
//! node, to check what `iota-tool admin` prints to each stream and its exit
//! code.

use std::{
    net::SocketAddr,
    process::{Command, Output, Stdio},
    sync::Arc,
};

use iota_core::authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder};
use iota_node::admin::traffic_control_router;
use iota_types::traffic_control::{PolicyConfig, TrafficControlStatus};

/// Serves the traffic control routes of a test node on a free port of the
/// loopback address, and returns their URL and the node.
async fn start_admin_server() -> (String, Arc<AuthorityState>) {
    let state = TestAuthorityBuilder::new()
        .with_policy_config(PolicyConfig {
            dry_run: false,
            ..PolicyConfig::default_dos_protection_policy()
        })
        .build()
        .await;
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = traffic_control_router(state.clone());
    tokio::spawn(async move { axum::serve(listener, router).await });
    (url, state)
}

/// What the `iota-tool` binary printed, and its exit code.
struct Printed {
    stdout: String,
    stderr: String,
    output: Output,
}

/// Runs `iota-tool admin --admin-url <url>` with `args`, with no input.
async fn run_iota_tool_admin(url: &str, args: &[&str]) -> Printed {
    let mut command = Command::new(env!("CARGO_BIN_EXE_iota-tool"));
    command
        .args(["admin", "--admin-url", url])
        .args(args)
        .stdin(Stdio::null());
    let output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    Printed {
        stdout: String::from_utf8(output.stdout.clone()).unwrap(),
        stderr: String::from_utf8(output.stderr.clone()).unwrap(),
        output,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_prints_the_settings_to_stdout_and_the_answer_of_the_node_to_stderr() {
    let (url, state) = start_admin_server().await;
    let printed =
        run_iota_tool_admin(&url, &["traffic-control", "set", "--spam-threshold", "500"]).await;
    let Printed { stdout, stderr, .. } = &printed;
    assert!(printed.output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("spam threshold"), "{stdout}");
    assert!(!stdout.contains("Traffic control configured"), "{stdout}");
    assert!(
        stderr.contains("Traffic control configured with:"),
        "{stderr}"
    );
    assert!(
        stderr.contains("iota-tool admin traffic-control save"),
        "{stderr}"
    );
    assert_eq!(
        state.traffic_control_status().unwrap().spam_threshold,
        Some(500)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_prints_only_json_to_stdout() {
    let (url, state) = start_admin_server().await;
    let printed = run_iota_tool_admin(
        &url,
        &[
            "--json",
            "traffic-control",
            "set",
            "--connection-blocklist-ttl-sec",
            "61",
        ],
    )
    .await;
    let Printed { stdout, stderr, .. } = &printed;
    assert!(printed.output.status.success(), "{stdout}{stderr}");
    assert_eq!(
        serde_json::from_str::<TrafficControlStatus>(stdout).unwrap(),
        state.traffic_control_status().unwrap()
    );
    assert!(
        stderr.contains("Traffic control configured with:"),
        "{stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_request_ends_with_exit_code_1_and_the_answer_of_the_node() {
    let (url, _) = start_admin_server().await;
    let printed = run_iota_tool_admin(
        &url,
        &[
            "traffic-control",
            "block",
            "203.0.113.9",
            "--ttl",
            "9999999",
        ],
    )
    .await;
    let Printed { stdout, stderr, .. } = &printed;
    assert_eq!(printed.output.status.code(), Some(1), "{stdout}{stderr}");
    // `exit_main!` prints the error to stdout.
    assert!(
        stdout.contains("the node answered 400 Bad Request"),
        "{stdout}{stderr}"
    );
}
