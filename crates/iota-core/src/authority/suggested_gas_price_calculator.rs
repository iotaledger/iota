use iota_types::executable_transaction::VerifiedExecutableTransaction;

use super::shared_object_congestion_info::MultiCommitCongestionInfo;

/// Suggested gas price calculator is a component that calculates suggested
/// gas prices for shared-object transactions.
pub(crate) struct SuggestedGasPriceCalculator;

impl SuggestedGasPriceCalculator {
    /// Calculate a suggested gas price using multi-commit congestion data
    /// for a given certificate. The reference gas price will be used as
    /// a suggested gas price if the certificate does not contain shared
    /// objects or if there is no congestion on the certificate's input
    /// shared objects, even though this function should only be called
    /// for certificate cancelled due to shared object congestion.
    pub fn calculate(
        _multi_commit_congestion_info: &MultiCommitCongestionInfo,
        certificate: &VerifiedExecutableTransaction,
        reference_gas_price: u64,
    ) -> u64 {
        if certificate.contains_shared_object() {
            // TODO: think about inflated gas price: there is no congestion, but senders set
            // high gas prices. The calculator should return reference gas price.

            reference_gas_price + 1
        } else {
            reference_gas_price
        }
    }
}
