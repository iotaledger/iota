// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 4 --addresses Test=0x0 --accounts A --simulator

//# run-graphql

{
  chainIdentifier @deprecated
}

//# run-graphql

fragment Modules on Object @deprecated {
    address
    asMovePackage {
        module(name: "m") {
            name
            package { address }

            fileFormatVersion
            bytes
            disassembly
        }
    }
}

{
    transactionBlocks(last: 1) {
        nodes {
            effects {
                objectChanges {
                  nodes {
                    outputState {
                        ...Modules
                    }
                  }
                }
            }
        }
    }
}

//# run-graphql

query($id: IotaAddress! @deprecated) {
  object(id: $id) {
    address
  }
}

//# run-graphql

{
  chainIdentifier @skip(if: true)
}

//# run-graphql

{
  chainIdentifier @skip(if: false)
}

//# run-graphql

{
  chainIdentifier @include(if: true)
}

//# run-graphql

{
  chainIdentifier @include(if: false)
}
