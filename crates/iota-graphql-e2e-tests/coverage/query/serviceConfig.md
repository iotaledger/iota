Query: `serviceConfig`

```
{
  serviceConfig {
    isEnabled(feature:COINS)
    availableVersions
    enabledFeatures
    maxQueryDepth
    maxQueryNodes
    maxOutputNodes
    maxDbQueryCost
    defaultPageSize
    maxPageSize
    mutationTimeoutMs
    requestTimeoutMs
    maxQueryPayloadSize
    maxTypeArgumentDepth
    maxTypeArgumentWidth
    maxTypeNodes
    maxMoveValueDepth
    maxTransactionIds
    maxScanLimit
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/call/simple.move](../../../iota-graphql-e2e-tests/tests/call/simple.move):

```
//# run-graphql
{
  serviceConfig {
    availableVersions
  }
}
```
