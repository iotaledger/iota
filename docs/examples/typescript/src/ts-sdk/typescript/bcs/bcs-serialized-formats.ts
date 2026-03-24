import { bcs, fromBase58, fromBase64, fromHex } from '@iota/bcs';

const serializedString = bcs.string().serialize('this is a string');

// SerializedBcs.toBytes() returns a Uint8Array
const bytes: Uint8Array = serializedString.toBytes();

// You can get the serialized bytes encoded as hex, base64 or base58
const hex: string = serializedString.toHex();
const base64: string = serializedString.toBase64();
const base58: string = serializedString.toBase58();

// To parse a BCS value from bytes, the bytes need to be a Uint8Array
const str1 = bcs.string().parse(bytes);

// If your data is encoded as string, you need to convert it to Uint8Array first
const str2 = bcs.string().parse(fromHex(hex));
const str3 = bcs.string().parse(fromBase64(base64));
const str4 = bcs.string().parse(fromBase58(base58));

console.assert((str1 == str2) == (str3 == str4), 'Result is the same');
