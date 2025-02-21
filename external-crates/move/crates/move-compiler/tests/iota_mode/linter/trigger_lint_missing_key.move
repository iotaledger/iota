module a::trigger_lint_cases {
    use iota::object::UID;

    // This should trigger the linter warning (true positive)
    struct MissingKeyAbility {
        id: UID,
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
}
