module reviews_rating::dashboard {
    use std::string::String;

    /// Assuming IOTA provides dynamic field-like behavior
    use iota::storage as storage;

    /// Dashboard is a collection of services
    public struct Dashboard has key, store {
        id: UID,
        service_type: String,
    }

    /// Create a new dashboard
    public fun create_dashboard(
        service_type: String,
        ctx: &mut TxContext,
    ) {
        let db = Dashboard {
            id: iota::object::new(ctx),  // IOTA object creation
            service_type,
        };
        iota::transfer::share_object(db);  // Sharing object via IOTA equivalent
    }

    /// Register a service with the dashboard
    public fun register_service(db: &mut Dashboard, service_id: ID) {
        storage::add(&mut db.id, service_id, service_id);  // Replacing dynamic fields with IOTA storage mechanism
    }
}
