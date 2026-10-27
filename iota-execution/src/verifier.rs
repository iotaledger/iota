// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    error::{IotaError, IotaResult},
    execution_config_utils::to_binary_config,
};
use move_binary_format::{CompiledModule, binary_config::BinaryConfig};
use move_bytecode_verifier_meter::Meter;
use move_vm_config::verifier::MeterConfig;

pub trait Verifier {
    /// Create a new bytecode verifier meter.
    fn meter(&self, config: MeterConfig) -> Box<dyn Meter>;

    /// Whether `meter_module_bytes` rejects a module that uses the deprecated
    /// global storage instructions, rather than leaving it to execution.
    fn deprecate_global_storage_ops(&self) -> bool;

    /// Run the bytecode verifier with a meter limit
    ///
    /// This function only fails if the verification does not complete within
    /// the limit.  If the modules fail to verify but verification completes
    /// within the meter limit, the function succeeds.
    fn meter_compiled_modules(
        &mut self,
        protocol_config: &ProtocolConfig,
        modules: &[CompiledModule],
        meter: &mut dyn Meter,
    ) -> IotaResult<()>;

    fn meter_module_bytes(
        &mut self,
        protocol_config: &ProtocolConfig,
        module_bytes: &[Vec<u8>],
        meter: &mut dyn Meter,
    ) -> IotaResult<()> {
        let deserialize = |binary_config: &BinaryConfig| {
            module_bytes
                .iter()
                .map(|b| CompiledModule::deserialize_with_config(b, binary_config))
                .collect::<Result<Vec<_>, _>>()
        };
        // The metered verifier does not support the deprecated global storage
        // instructions, so they are disallowed here whoever calls.
        let modules = match deserialize(&to_binary_config(protocol_config, Some(true))) {
            Ok(modules) => modules,
            Err(error) => {
                // The modules fail only because they use the deprecated global
                // storage instructions or a non-empty `acquires` list. No such
                // module can be published, so reject it here rather than at
                // execution.
                if self.deprecate_global_storage_ops()
                    && deserialize(&to_binary_config(protocol_config, Some(false))).is_ok()
                {
                    return Err(IotaError::ModuleDeserializationFailure {
                        error: error.to_string(),
                    });
                }
                // Although we failed, we don't care since it wasn't because of a timeout.
                return Ok(());
            }
        };

        for module in &modules {
            for identifier in module.identifiers() {
                if identifier.as_str() == "<SELF>" {
                    return Err(iota_types::error::UserInputError::InvalidIdentifier {
                        error: format!("invalid identifier: {identifier}"),
                    }
                    .into());
                }
            }
        }

        self.meter_compiled_modules(protocol_config, &modules, meter)
    }
}
