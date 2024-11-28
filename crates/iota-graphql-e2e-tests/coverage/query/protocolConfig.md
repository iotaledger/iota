Query: `protocolConfig`

```graphql
{
  protocolConfig(protocolVersion: 1) {
    protocolVersion
    featureFlags {
      key
      value
      __typename
    }
    configs {
      key
      value
      __typename
    }
    config(key: "address_from_bytes_cost_base") {
      key
      value
      __typename
    }
    featureFlag(key: "bridge") {
      key
      value
      __typename
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/epoch/protocol_configs.move](../../../iota-graphql-e2e-tests/tests/epoch/protocol_configs.move):

```graphql
//# run-graphql
{
    protocolConfig {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "bridge") {
            value
        }
    }
}

//# run-graphql
{
    protocolConfig(protocolVersion: 1) {
        protocolVersion
        config(key: "max_move_identifier_len") {
            value
        }
        featureFlag(key: "bridge") {
            value
        }
    }
}
```
