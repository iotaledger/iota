// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::{error::IotaResult, execution_config_utils::to_binary_config};
use move_binary_format::CompiledModule;
use move_bytecode_verifier_meter::Meter;
use move_vm_config::verifier::MeterConfig;

pub trait Verifier {
    /// Create a new bytecode verifier meter.
    fn meter(&self, config: MeterConfig) -> Box<dyn Meter>;

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
        let binary_config = to_binary_config(protocol_config);
        let Ok(modules) = module_bytes
            .iter()
            .map(|b| CompiledModule::deserialize_with_config(b, &binary_config))
            .collect::<Result<Vec<_>, _>>()
        else {
            // Although we failed, we don't care since it wasn't because of a timeout.
            return Ok(());
        };

        self.meter_compiled_modules(protocol_config, &modules, meter)
    }
}
