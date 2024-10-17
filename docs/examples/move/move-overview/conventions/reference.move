module conventions::profile {

    use std::string::String;

    public struct Profile has key {
        id: UID,
        name: String,
        age: u8
    }

    // profile.name()
    public fun name(self: &Profile): &String {
        &self.name
    }

    // profile.age_mut()
    public fun age_mut(self: &mut Profile): &mut u8 {
        &mut self.age
    }
}
