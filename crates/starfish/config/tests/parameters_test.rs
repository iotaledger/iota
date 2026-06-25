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
fn protective_preset_enables_bounds_and_default_stays_inert() {
    let protective = starfish_config::TonicParameters::protective();
    assert_eq!(protective.max_concurrent_streams, 64);
    assert_eq!(
        protective.request_timeout,
        std::time::Duration::from_secs(120)
    );
    assert_eq!(protective.max_inbound_message_size, 1 << 20);
    assert_eq!(protective.admission.max_subscriptions_per_peer, 2);
    assert_eq!(protective.admission.max_header_fetches_per_peer, 32);
    assert_eq!(protective.admission.max_transaction_fetches_per_peer, 16);
    assert!(protective.admission.max_commit_fetches_per_peer > 0);

    // The default must stay inert so behaviour is unchanged unless opted in.
    let inert = starfish_config::TonicParameters::default();
    assert_eq!(inert.max_concurrent_streams, 0);
    assert!(inert.request_timeout.is_zero());
    assert_eq!(inert.max_inbound_message_size, 0);
    assert_eq!(inert.admission.max_header_fetches_per_peer, 0);
}

#[test]
fn apply_protective_preserves_operator_transport_fields() {
    // Operator-customised transport settings that must survive opting in.
    let mut tonic = starfish_config::TonicParameters {
        keepalive_interval: std::time::Duration::from_secs(11),
        connection_buffer_size: 9 << 20,
        excessive_message_size: 5 << 20,
        message_size_limit: 7 << 20,
        ..Default::default()
    };

    tonic.apply_protective();

    // Protective bounds applied.
    assert_eq!(tonic.max_concurrent_streams, 64);
    assert_eq!(tonic.max_inbound_message_size, 1 << 20);
    assert_eq!(tonic.admission.max_header_fetches_per_peer, 32);
    // Operator transport settings preserved (not reset to defaults).
    assert_eq!(tonic.keepalive_interval, std::time::Duration::from_secs(11));
    assert_eq!(tonic.connection_buffer_size, 9 << 20);
    assert_eq!(tonic.excessive_message_size, 5 << 20);
    assert_eq!(tonic.message_size_limit, 7 << 20);
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
