module conventions::profile {

    use iota::transfer::share_object;

    public struct Profile has key {
        id: UID
    }

    public fun new(ctx:&mut TxContext): Profile {
        Profile {
            id: object::new(ctx)
        }
    }

    public fun share(profile: Profile) {
        share_object(profile);
    }
}
