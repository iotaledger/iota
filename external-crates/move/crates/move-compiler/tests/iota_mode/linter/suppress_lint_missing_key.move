module a::trigger_lint_cases {
    use iota::object::UID;

    // 4. Suppress warning
    #[allow(lint(missing_key))]
    struct SuppressWarning {
       id: UID,
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
