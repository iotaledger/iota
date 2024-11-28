Query: `coinMetadata`

```
{
  coinMetadata(coinType: "@{test}::fake::FAKE") {
    digest
    storageRebate
    bcs
    decimals
    name
    symbol
    description
    iconUrl
    supply
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/call/coin_metadata.move](crates/iota-graphql-e2e-tests/tests/call/coin_metadata.move):

```
//# run-graphql
{
  coinMetadata(coinType: "@{test}::fake::FAKE") {
    decimals
    name
    symbol
    description
    iconUrl
    supply
  }
}

//# run-graphql
{
  coinMetadata(coinType: "@{test}::fake::FAKE") {
    decimals
    name
    symbol
    description
    iconUrl
    supply
  }
}
```
