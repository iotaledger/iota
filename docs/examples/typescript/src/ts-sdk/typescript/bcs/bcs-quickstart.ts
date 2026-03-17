import { bcs, fromHex, toHex } from '@iota/bcs';

// define UID as a 32-byte array, then add a transform to/from hex strings
const UID = bcs.fixedArray(32, bcs.u8()).transform({
    input: (id: string) => fromHex(id),
    output: (id: number[]) => toHex(Uint8Array.from(id)),
});

const Coin = bcs.struct('Coin', {
    id: UID,
    value: bcs.u64(),
});

// deserialization: BCS bytes into Coin
const bcsBytes = Coin.serialize({
    id: '0000000000000000000000000000000000000000000000000000000000000001',
    value: 1000000n,
}).toBytes();

const coin = Coin.parse(bcsBytes);

// serialization: Object into bytes - an Option with <T = Coin>
const hex = bcs.option(Coin).serialize(coin).toHex();

console.log(hex);
