use iota_sdk::types::block::address::Address;
use sui_types::base_types::SuiAddress;

/// Converts a ["Stardust" `Address`](Address) to a [`SuiAddress`].
pub fn stardust_to_sui_address(stardust_address: impl Into<Address>) -> anyhow::Result<SuiAddress> {
    stardust_address.into().to_string().parse()
}
