// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.command.rs");
include!("../../../generated/iota.grpc.v0.command.field_info.rs");

impl From<iota_sdk_types::transaction::Argument> for Argument {
    fn from(arg: iota_sdk_types::transaction::Argument) -> Self {
        let kind = match arg {
            iota_sdk_types::transaction::Argument::Gas => {
                argument::Kind::GasCoin(argument::GasCoin {})
            }
            iota_sdk_types::transaction::Argument::Input(idx) => {
                argument::Kind::Input(argument::Input {
                    index: Some(idx as u32),
                })
            }
            iota_sdk_types::transaction::Argument::Result(idx) => {
                argument::Kind::Result(argument::Result {
                    index: Some(idx as u32),
                    nested_result_index: None,
                })
            }
            iota_sdk_types::transaction::Argument::NestedResult(idx, nested_idx) => {
                argument::Kind::Result(argument::Result {
                    index: Some(idx as u32),
                    nested_result_index: Some(nested_idx as u32),
                })
            }
            _ => panic!("Unsupported argument type for gRPC conversion"),
        };

        Self { kind: Some(kind) }
    }
}
