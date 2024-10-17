module examples::transferable_witness {

    /// A witness struct that is storable and droppable.
    public struct WITNESS has store, drop {}

    /// A carrier that holds the WITNESS and can be transferred.
    public struct WitnessCarrier has key {
        id: UID,
        witness: WITNESS
    }

    /// Initializes the module by sending a WitnessCarrier to the module publisher.
    fun init(ctx: &mut TxContext) {
        transfer::transfer(
            WitnessCarrier {
                id: object::new(ctx),
                witness: WITNESS {}
            },
            tx_context::sender(ctx)
        )
    }

    /// Extracts the WITNESS from the WitnessCarrier, consuming the carrier.
    public fun get_witness(carrier: WitnessCarrier): WITNESS {
        let WitnessCarrier { id, witness } = carrier;
        object::delete(id);
        witness
    }
}
