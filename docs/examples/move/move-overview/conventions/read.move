module conventions::profile {

    public struct Profile {
        age: u64
    }

    // ✅ Correct
    public fun age(self: &Profile): u64 {
        self.age
    }

    // ❌ Incorrect
    public fun profile_age(self: &Profile): u64 {
        self.age
    }
}

module conventions::defi {

    use conventions::profile::Profile;

    public fun get_tokens(profile: &Profile) {

        // ✅ Correct
        let name = profile.age();

        // ❌ Incorrect
        let name2 = profile.profile_age();
    }
}
