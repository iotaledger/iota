import { bcs, fromHex, toHex } from '@iota/bcs';

const Address = bcs.bytes(32).transform({
    // To change the input type, you need to provide a type definition for the input
    input: (val: string) => fromHex(val),
    output: (val: Uint8Array) => toHex(val),
});

const serialized = Address.serialize('0x0000000000000000000000000000000000000000000000000000000000000000').toBytes();
const parsed = Address.parse(serialized); // will return a hex string

console.log({ serialized, parsed });
