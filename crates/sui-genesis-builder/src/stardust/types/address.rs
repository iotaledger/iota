use iota_sdk::types::block::address::Address;
use sui_types::{base_types::SuiAddress, object::Owner};

/// Converts a ["Stardust" `Address`](Address) to a [`SuiAddress`].
///
/// This is intended as the only conversion function to go from Stardust to Sui addresses, so there is only
/// one place to potentially update it if we decide to change it later.
pub fn stardust_to_sui_address(stardust_address: impl Into<Address>) -> anyhow::Result<SuiAddress> {
    stardust_address.into().to_string().parse()
}

/// Converts a ["Stardust" `Address`](Address) to a [`SuiAddress`] and then wraps it into an [`Owner`]
/// which is either address- or object-owned depending on the stardust address.
pub fn stardust_to_sui_address_owner(
    stardust_address: impl Into<Address>,
) -> anyhow::Result<Owner> {
    stardust_to_sui_address(stardust_address.into()).map(Owner::AddressOwner)
}
