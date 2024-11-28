Query: `type`

```
{
  type(type: "vector<u64>") {
    repr
    signature
    layout
    abilities
    __typename
  }
}
```

tested by [crates/iota-graphql-e2e-tests/coverage/query/type.md](../../../iota-graphql-e2e-tests/coverage/query/type.md):

```
//# run-graphql
# Happy path -- primitive type with generic parameter

{
    type(type: "vector<u64>") {
        repr
        signature
        layout
    }
}
```
