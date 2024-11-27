Query: `availableRange`

```
{
  availableRange {
    first {
      digest
    }
    last {
      digest
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/available_range/available_range.move](crates/iota-graphql-e2e-tests/tests/available_range/available_range.move):

```
//# run-graphql
{
  availableRange {
    first {
      digest
      sequenceNumber
    }
    last {
      digest
      sequenceNumber
    }
  }

  first: checkpoint(id: { sequenceNumber: 0 } ) {
    digest
    sequenceNumber
  }

  last: checkpoint {
    digest
    sequenceNumber
  }
}
```
