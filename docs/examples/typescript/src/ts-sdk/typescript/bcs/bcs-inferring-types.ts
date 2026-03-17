import { bcs, type InferBcsType, type InferBcsInput } from '@iota/bcs';

const MyStruct = bcs.struct('MyStruct', {
    id: bcs.u64(),
    name: bcs.string(),
});

// using the $inferType and $inferInput properties
type MyStructType = typeof MyStruct.$inferType; // { id: string; name: string; }
type MyStructInput = typeof MyStruct.$inferInput; // { id: number | string | bigint; name: string; }

// using the InferBcsType and InferBcsInput type helpers
type MyStructType2 = InferBcsType<typeof MyStruct>; // { id: string; name: string; }
type MyStructInput2 = InferBcsInput<typeof MyStruct>; // { id: number | string | bigint; name: string; }

// Suppress unused type warnings
export type { MyStructType, MyStructInput, MyStructType2, MyStructInput2 };
