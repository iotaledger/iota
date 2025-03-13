// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0


//# init --protocol-version 4 --addresses Test=0x0 --accounts A --simulator

//# publish --upgradeable --sender A
module Test::M0 {
  public struct Event has copy, drop {
    value: u64
  }
}
//# upgrade --package Test --upgrade-capability 1,1 --sender A
module Test::M0 {
  public struct Event has copy, drop {
    value: u64
  }
  public fun emit() {
    iota::event::emit(Event { value: 42 })
  }
}
module Test::M1 {
  public fun emit() {
    Test::M0::emit()
  }
}
//# run Test::M1::emit --sender A

//# create-checkpoint

//# run-graphql
{
  events {
    nodes {
      sendingModule {
        package {
          address
        }
        name
      }
      type {
        repr
      }
    }
  }
}