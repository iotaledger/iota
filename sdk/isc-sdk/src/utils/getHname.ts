import { blake2b } from '@noble/hashes/blake2';

export function getHname(input: string): number {
    // Encode input as UTF-8 and hash it using BLAKE2b (32-byte digest)
    const hash = blake2b(new TextEncoder().encode(input), { dkLen: 32 });

    // Extract the first 4 bytes and interpret as little-endian uint32
    const value = hash[0] | (hash[1] << 8) | (hash[2] << 16) | (hash[3] << 24);

    return Number('0x' + (value >>> 0).toString(16).padStart(8, '0')); // Convert to hex with leading zeros
}
