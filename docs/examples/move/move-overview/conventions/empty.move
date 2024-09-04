module conventions::collection {

    public struct Collection has copy, drop, store {
        bits: vector<u8>
    }

    public fun empty(): Collection {
        Collection {
            bits: vector[]
        }
    }
}
