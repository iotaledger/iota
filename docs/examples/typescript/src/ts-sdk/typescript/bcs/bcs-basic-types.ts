import { bcs } from '@iota/bcs';

// Integers
const u8 = bcs.u8().serialize(100).toBytes();
const u64 = bcs.u64().serialize(1000000n).toBytes();
const u128 = bcs.u128().serialize('100000010000001000000').toBytes();

// Other types
const str = bcs.string().serialize('this is an ascii string').toBytes();
const bytes = bcs.bytes(4).serialize([1, 2, 3, 4]).toBytes();

const byteVector = bcs
    .byteVector()
    .serialize(new Uint8Array([1, 2, 3, 4]))
    .toBytes();

// Parsing data back into original types
const parsedU8 = bcs.u8().parse(u8);
// u64-u256 will be represented as bigints regardless of how they were provided when serializing them
const parsedU64 = bcs.u64().parse(u64);
const parsedU128 = bcs.u128().parse(u128);

const parsedStr = bcs.string().parse(str);
const parsedBytes = bcs.bytes(4).parse(bytes);
const parsedByteVector = bcs.byteVector().parse(byteVector);

console.log({ parsedU8, parsedU64, parsedU128, parsedStr, parsedBytes, parsedByteVector });
