module examples::lock_and_key {
    /// Error indicating that the Lock is empty and there is nothing to retrieve.
    const ELockIsEmpty: u64 = 0;

    /// Error indicating that the Key does not match the Lock.
    const EKeyMismatch: u64 = 1;

    /// Error indicating that the Lock already contains an item.
    const ELockIsFull: u64 = 2;

    /// A Lock that can store any content within it.
    public struct Lock<T: store + key> has key {
        id: UID,
        locked: Option<T>
    }

    /// A Key that is created alongside a Lock; it is transferable and contains all the necessary information to unlock the Lock.
    public struct Key<phantom T: store + key> has key, store {
        id: UID,
        key_for: ID,
    }

    /// Retrieves the ID of the Lock associated with a given Key.
    public fun key_for<T: store + key>(key: &Key<T>): ID {
        key.key_for
    }

    /// Locks content within a shared object. A Key is generated and sent to the transaction sender.
    /// For example, you could lock some `Coin<IOTA>` within a Lock to create a treasure chest.
    /// The sender receives the Key to this Lock.
    public fun create<T: store + key>(obj: T, ctx: &mut TxContext): Key<T> {
        let id = object::new(ctx);
        let key_for = object::uid_to_inner(&id);

        transfer::share_object(Lock<T> {
            id,
            locked: option::some(obj),
        });

        let key = Key<T> {
            key_for,
            id: object::new(ctx)
        };
        key
    }

    /// Locks an item within a shared object using a Key. Aborts if the Lock is not empty or if the Key does not match the Lock.
    public fun lock<T: store + key>(
        obj: T,
        lock: &mut Lock<T>,
        key: &Key<T>,
    ) {
        assert!(option::is_none(&lock.locked), ELockIsFull);
        assert!(&key.key_for == object::borrow_id(lock), EKeyMismatch);

        option::fill(&mut lock.locked, obj);
    }

    /// Unlocks a Lock with a Key to access its contents.
    /// This function can only be called if:
    /// - The Key matches the Lock.
    /// - The Lock is not empty.
    public fun unlock<T: store + key>(
        lock: &mut Lock<T>,
        key: &Key<T>,
    ): T {
        assert!(option::is_some(&lock.locked), ELockIsEmpty);
        assert!(&key.key_for == object::borrow_id(lock), EKeyMismatch);

        option::extract(&mut lock.locked)
    }

    /// Unlocks the Lock and transfers its contents to the transaction sender.
    public fun take<T: store + key>(
        lock: &mut Lock<T>,
        key: &Key<T>,
        ctx: &mut TxContext,
    ) {
        transfer::public_transfer(unlock(lock, key), tx_context::sender(ctx))
    }
}
