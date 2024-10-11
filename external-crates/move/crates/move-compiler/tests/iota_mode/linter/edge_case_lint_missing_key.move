module a::edge_cases {
    struct UID {}
    // Test case with a different UID type
    struct DifferentUID {
        id: iota::another::UID,
    }

    struct NotAnObject {
        id: UID,
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::another {
    struct UID has store {
        id: address,
    }
}
