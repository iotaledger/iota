// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 17 --addresses Test=0x0 --accounts A --simulator

//# publish
module Test::M1 {
    use iota::event;

    public struct EventA has copy, drop {
        new_value: u64
    }

    public entry fun emit_100() {
        let mut i = 0;
        while (i < 100) {
            event::emit(EventA { new_value: i });
            i = i + 1;
        }
    }
}

//# run Test::M1::emit_100 --sender A

//# create-checkpoint

//# run-graphql
{
  events(filter: { sender: "@{A}" }) {
    nodes {
      sendingModule {
        name
      }
      json
      bcs
      transactionBlock {
          digest
      }
    }
  }
}

//# run-graphql
# This should fail due to the complexity of the query.
{
  events(first: 50, filter: { sender: "@{A}" }) {
    nodes {
      sendingModule {
        name
      }
      json
      bcs
      transactionBlock {
          digest
          effects {
              events(first: 50) {
                  nodes {
                      transactionBlock {
                          digest
                      }
                  }
              }
          }
      }
    }
  }
}

//# run-graphql
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        events {
          edges {
            node {
              sendingModule {
                name
              }
            }
          }
        }
      }
    }
  }
}

//# run-graphql
# This should fail due to the complexity of the query
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        events {
          edges {
            node {
              sendingModule {
                name
              }
              transactionBlock {
                digest
              }
            }
          }
        }
      }
    }
  }
}

//# run-graphql
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        dependencies {
            nodes {
                digest
                effects {
                    events {
                        edges {
                            node {
                                sendingModule {
                                    name
                                }
                            }
                        }
                    }
                }
            }
        }
        events {
          edges {
            node {
              sendingModule {
                name
              }
            }
          }
        }
      }
    }
  }
}

//# run-graphql
# This should fail due to the complexity of the query
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        dependencies {
            nodes {
                digest
                effects {
                    events {
                        edges {
                            node {
                                sendingModule {
                                    name
                                }
                                transactionBlock {
                                    digest
                                }
                            }
                        }
                    }
                }
            }
        }
        events {
          edges {
            node {
              sendingModule {
                name
              }
            }
          }
        }
      }
    }
  }
}

//# run-graphql
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        checkpoint {
            transactionBlocks {
                nodes {
                  digest
                }
            }
        }
        events {
          edges {
            node {
              sendingModule {
                name
              }
            }
          }
        }
      }
    }
  }
}

//# run-graphql
# This should fail due to the complexity of the query
{
  transactionBlocks(filter: { signAddress: "@{A}" }) {
    nodes {
      effects{
        checkpoint {
            transactionBlocks {
                nodes {
                  digest
                  effects{
                    events {
                      edges {
                        node {
                          sendingModule {
                            name
                          }
                          transactionBlock {
                            digest
                          }
                        }
                      }
                    }
                  }
                }
            }
        }
        events {
          edges {
            node {
              sendingModule {
                name
              }
            }
          }
        }
      }
    }
  }
}
