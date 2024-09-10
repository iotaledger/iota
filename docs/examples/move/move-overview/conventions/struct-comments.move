module conventions::profile {
    use std::string::String;

    public struct Profile has key, store {
        id: UID,
        /// The age of the user
        age: u8,
        /// The first name of the user
        name: String
    }
}
