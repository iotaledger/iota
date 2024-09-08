/// This example illustrates how a One-Time Witness (OTW) functions.
///
/// A One-Time Witness (OTW) is an instance of a type guaranteed to be
/// unique within the system. Its properties include:
///
/// - Created solely in the module initializer.
/// - Named after the module (in uppercase).
/// - Cannot be manually instantiated.
/// - Possesses the `drop` ability.
module examples::one_time_witness_registry {
    use std::string::String;

    // Dependency to check whether a type is a One-Time Witness (OTW)
    use iota::types;

    /// Error code for non-OTW structures
    const ENotOneTimeWitness: u64 = 0;

    /// A record to mark the existence of a unique type, ensuring only one instance per type.
    public struct UniqueTypeRecord<phantom T> has key {
        id: UID,
        name: String
    }

    /// Public function to register new types with custom names.
    /// The `is_one_time_witness` function ensures that this function
    /// can only be called once for a specific `T`.
    public fun add_record<T: drop>(
        witness: T,
        name: String,
        ctx: &mut TxContext
    ) {
        // Verify that the type is an OTW
        assert!(types::is_one_time_witness(&witness), ENotOneTimeWitness);

        // Share the record globally
        transfer::share_object(UniqueTypeRecord<T> {
            id: object::new(ctx),
            name
        });
    }
}

/// Example of creating a One-Time Witness (OTW).
module examples::my_otw {
    use std::string;
    use examples::one_time_witness_registry as registry;

    /// The type name is the uppercase version of the module name
    public struct MY_OTW has drop {}

    /// The OTW instance is provided as the first argument in the module initializer.
    /// This instance is a full value, not a reference.
    fun init(witness: MY_OTW, ctx: &mut TxContext) {
        registry::add_record(
            witness, // OTW instance passed here
            string::utf8(b"My awesome record"),
            ctx
        )
    }
}
