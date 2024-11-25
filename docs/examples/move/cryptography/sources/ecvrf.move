module example::ecvrf {
    use iota::ecvrf;
    use iota::event;

    /// Event on whether the output is verified
    public struct VerifiedEvent has copy, drop {
        is_verified: bool,
    }

    public fun verify_ecvrf_output(output: vector<u8>, alpha_string: vector<u8>, public_key: vector<u8>, proof: vector<u8>) {
        event::emit(VerifiedEvent {is_verified: ecvrf::ecvrf_verify(&output, &alpha_string, &public_key, &proof)});
    }
}