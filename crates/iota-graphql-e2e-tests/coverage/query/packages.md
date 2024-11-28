Query: `package`

```graphql
{
  packages(first: null, last: 5, after: null, filter: null) {
    edges {
      node {
        address
        objects {
          edges {
            node {
              digest
              storageRebate
              bcs
            }
          }
        }
        balance {
          coinObjectCount
          totalBalance
        }
        balances {
          edges {
            node {
              coinObjectCount
              totalBalance
            }
          }
        }
        coins {
          edges {
            node {
              digest
              storageRebate
              bcs
              coinBalance
            }
          }
        }
        stakedIotas {
          edges {
            node {
              digest
              storageRebate
              bcs
              poolId
              principal
              estimatedReward
            }
          }
        }
        version
        status
        digest
        owner {
          __typename
        }
        previousTransactionBlock {
          digest
          bcs
        }
        storageRebate
        receivedTransactionBlocks {
          edges {
            node {
              digest
              bcs
            }
          }
        }
        bcs
        packageVersions {
          edges {
            node {
              digest
              storageRebate
              bcs
              moduleBcs
            }
          }
        }
        latestPackage {
          digest
          storageRebate
          bcs
          moduleBcs
        }
        module(name: "address") {
          bytes
          disassembly
        }
        modules {
          edges {
            node {
              name
              package {
                digest
                storageRebate
                bcs
                moduleBcs
              }
              datatype(name: "Char") {
                abilities
                asMoveEnum {
                  abilities
                  name
                  variants {
                    name
                    __typename
                  }
                  __typename
                  typeParameters {
                    isPhantom
                    __typename
                    constraints
                  }
                }
                asMoveStruct {
                  abilities
                  name
                  __typename
                  typeParameters {
                    isPhantom
                    __typename
                    constraints
                  }
                }
                name
                __typename
                typeParameters {
                  isPhantom
                  __typename
                  constraints
                }
              }
              datatypes {
                edges {
                  node {
                    abilities
                    asMoveEnum {
                      abilities
                      name
                      variants {
                        name
                        __typename
                      }
                      __typename
                      typeParameters {
                        isPhantom
                        __typename
                        constraints
                      }
                    }
                    asMoveStruct {
                      abilities
                      name
                      __typename
                      typeParameters {
                        isPhantom
                        __typename
                        constraints
                      }
                    }
                    name
                    __typename
                    typeParameters {
                      isPhantom
                      __typename
                      constraints
                    }
                  }
                }
              }
              __typename
              fileFormatVersion
              bytes
              disassembly
            }
          }
        }
        linkage {
          version
          __typename
        }
        typeOrigins {
          __typename
        }
        moduleBcs
      }
    }
  }
}
```

tested by [crates/iota-graphql-e2e-tests/tests/packages/versioning.move](../../../iota-graphql-e2e-tests/tests/packages/versioning.move):

```graphql
//# run-graphql
{   # Querying packages with checkpoint bounds
    before: packages(first: 10, filter: { beforeCheckpoint: 1 }) {
        nodes {
            address
            version
            previousTransactionBlock {
                effects { checkpoint { sequenceNumber } }
            }
        }
    }

    after: packages(first: 10, filter: { afterCheckpoint: 1 }) {
        nodes {
            address
            version
            previousTransactionBlock {
                effects { checkpoint { sequenceNumber } }
            }
        }
    }

    between: packages(first: 10, filter: { afterCheckpoint: 1, beforeCheckpoint: 3 }) {
        nodes {
            address
            version
            previousTransactionBlock {
                effects { checkpoint { sequenceNumber } }
            }
        }
    }
}
```
