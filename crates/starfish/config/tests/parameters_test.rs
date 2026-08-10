// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test]
#[cfg(not(msim))]
fn parameters_snapshot_matches() {
    let parameters = starfish_config::Parameters::default();
    insta::assert_yaml_snapshot!("parameters", parameters)
}

#[test]
fn default_enables_inbound_bounds() {
    let tonic = starfish_config::TonicParameters::default();
    assert_eq!(tonic.max_concurrent_streams, 64);
    assert_eq!(tonic.request_timeout, std::time::Duration::from_secs(120));
    assert_eq!(tonic.max_inbound_message_size, 1 << 20);
    assert_eq!(tonic.admission.max_subscriptions_per_peer, 2);
    assert_eq!(tonic.admission.max_header_fetches_per_peer, 32);
    assert_eq!(tonic.admission.max_transaction_fetches_per_peer, 16);
    assert!(tonic.admission.max_commit_fetches_per_peer > 0);
    assert_eq!(
        tonic.subscribe_request_timeout,
        std::time::Duration::from_secs(30)
    );
}

#[test]
fn operator_config_overrides_defaults() {
    let parameters: starfish_config::Parameters = serde_yaml::from_str(
        r#"
tonic:
  max_concurrent_streams: 128
  max_inbound_message_size: 4194304
  admission:
    max_header_fetches_per_peer: 5
    max_subscriptions_per_peer: 0
"#,
    )
    .unwrap();
    let defaults = starfish_config::Parameters::default();

    assert_eq!(parameters.tonic.max_concurrent_streams, 128);
    assert_eq!(parameters.tonic.max_inbound_message_size, 4 << 20);
    assert_eq!(parameters.tonic.admission.max_header_fetches_per_peer, 5);
    // A cap set to `0` disables admission for that group.
    assert_eq!(parameters.tonic.admission.max_subscriptions_per_peer, 0);
    // Unspecified fields keep their defaults, per field rather than per block.
    assert_eq!(
        parameters.tonic.admission.max_transaction_fetches_per_peer,
        defaults.tonic.admission.max_transaction_fetches_per_peer
    );
    assert_eq!(
        parameters.tonic.request_timeout,
        defaults.tonic.request_timeout
    );
    assert_eq!(
        parameters.tonic.keepalive_interval,
        defaults.tonic.keepalive_interval
    );
}

#[test]
fn partial_tonic_config_falls_back_to_defaults() {
    let parameters: starfish_config::Parameters = serde_yaml::from_str(
        r#"
tonic:
  keepalive_interval:
    secs: 5
    nanos: 0
  admission: {}
"#,
    )
    .unwrap();
    let defaults = starfish_config::Parameters::default();

    assert_eq!(
        parameters.tonic.max_concurrent_streams,
        defaults.tonic.max_concurrent_streams
    );
    assert_eq!(
        parameters.tonic.request_timeout,
        defaults.tonic.request_timeout
    );
    assert_eq!(
        parameters.tonic.max_inbound_message_size,
        defaults.tonic.max_inbound_message_size
    );
    assert_eq!(
        parameters.tonic.admission.max_subscriptions_per_peer,
        defaults.tonic.admission.max_subscriptions_per_peer
    );
    assert_eq!(
        parameters.tonic.admission.max_header_fetches_per_peer,
        defaults.tonic.admission.max_header_fetches_per_peer
    );
    assert_eq!(
        parameters.tonic.admission.max_transaction_fetches_per_peer,
        defaults.tonic.admission.max_transaction_fetches_per_peer
    );
    assert_eq!(
        parameters.tonic.admission.max_commit_fetches_per_peer,
        defaults.tonic.admission.max_commit_fetches_per_peer
    );
    assert_eq!(
        parameters.tonic.subscribe_request_timeout,
        defaults.tonic.subscribe_request_timeout
    );
}

#[test]
fn validate_accepts_defaults() {
    starfish_config::Parameters::default().validate().unwrap();
}

#[test]
fn validate_rejects_zero_values() {
    let parameters = starfish_config::Parameters {
        max_headers_per_bundle: 0,
        ..Default::default()
    };
    let error = parameters.validate().unwrap_err();
    assert!(error.contains("max_headers_per_bundle"));

    let parameters = starfish_config::Parameters {
        tonic: starfish_config::TonicParameters {
            keepalive_interval: std::time::Duration::ZERO,
            ..Default::default()
        },
        ..Default::default()
    };
    let error = parameters.validate().unwrap_err();
    assert!(error.contains("keepalive_interval"));

    // `excessive_message_size` is a pure metrics threshold with no "0 disables
    // it" meaning, so a zero would flag every message as excessive.
    let parameters = starfish_config::Parameters {
        tonic: starfish_config::TonicParameters {
            excessive_message_size: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let error = parameters.validate().unwrap_err();
    assert!(error.contains("excessive_message_size"));
}
