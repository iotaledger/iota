// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { secp256r1 } from '@noble/curves/p256';
import { blake2b } from '@noble/hashes/blake2b';
import { sha256 } from '@noble/hashes/sha256';
import { describe, expect, it } from 'vitest';

import { bcs } from '../../../src/bcs';
import { messageWithIntent } from '../../../src/cryptography';
import { PasskeyKeypair } from '../../../src/keypairs/passkey';
import { findCommonPublicKey, PasskeyProvider } from '../../../src/keypairs/passkey/keypair';
import {
    parseSerializedPasskeySignature,
    PasskeyPublicKey,
    SECP256R1_SPKI_HEADER,
} from '../../../src/keypairs/passkey/publickey';
import {
    AuthenticationCredential,
    RegistrationCredential,
} from '../../../src/keypairs/passkey/types';
import { fromBase64 } from '../../../src/utils';

function compressedPubKeyToDerSPKI(compressedPubKey: Uint8Array): Uint8Array {
    // Combine header with the uncompressed public key coordinates.
    const uncompressedPubKey =
        secp256r1.ProjectivePoint.fromHex(compressedPubKey).toRawBytes(false);
    return new Uint8Array([...SECP256R1_SPKI_HEADER, ...uncompressedPubKey]);
}

const mockAuthenticatorData = new Uint8Array([
    5, 255, 70, 6, 219, 66, 75, 122, 154, 227, 185, 96, 194, 195, 82, 112, 172, 117, 132, 208, 230,
    120, 204, 131, 54, 5, 234, 141, 222, 123, 76, 95, 1, 0, 0, 0, 0,
]);

class MockPasskeySigner implements PasskeyProvider {
    private sk: Uint8Array;
    private authenticatorData: Uint8Array;
    private pk: Uint8Array | null;
    private changeDigest: boolean;
    private changeClientDataJson: boolean;
    private changeAuthenticatorData: boolean;
    private changeSignature: boolean;

    constructor(options?: {
        sk?: Uint8Array;
        pk?: Uint8Array;
        authenticatorData?: Uint8Array;
        changeDigest?: boolean;
        changeClientDataJson?: boolean;
        changeAuthenticatorData?: boolean;
        changeSignature?: boolean;
    }) {
        this.sk = options?.sk ?? secp256r1.utils.randomPrivateKey();
        this.pk = options?.pk ?? null;
        this.authenticatorData = options?.authenticatorData ?? mockAuthenticatorData;
        this.changeDigest = options?.changeDigest ?? false;
        this.changeClientDataJson = options?.changeClientDataJson ?? false;
        this.changeAuthenticatorData = options?.changeAuthenticatorData ?? false;
        this.changeSignature = options?.changeSignature ?? false;
    }

    async create(): Promise<RegistrationCredential> {
        const pk = this.pk;
        const credentialResponse: AuthenticatorAttestationResponse = {
            attestationObject: new Uint8Array().slice().buffer,
            clientDataJSON: new TextEncoder()
                .encode(
                    JSON.stringify({
                        type: 'webauthn.create',
                        challenge: '',
                        origin: 'https://www.iota.org',
                        crossOrigin: false,
                    }),
                )
                .slice().buffer,
            getPublicKey: () =>
                pk
                    ? compressedPubKeyToDerSPKI(pk).slice().buffer
                    : new Uint8Array([
                          48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206,
                          61, 3, 1, 7, 3, 66, 0, 4, 150, 14, 177, 148, 129, 92, 179, 77, 32, 147,
                          160, 100, 10, 70, 62, 88, 35, 97, 203, 9, 152, 174, 47, 126, 59, 185, 217,
                          4, 103, 35, 139, 198, 0, 115, 207, 97, 251, 66, 114, 9, 49, 140, 24, 141,
                          189, 167, 45, 10, 99, 115, 155, 55, 107, 64, 72, 60, 149, 208, 198, 252,
                          60, 223, 215, 229,
                      ]).slice().buffer,
            getPublicKeyAlgorithm: () => -7,
            getTransports: () => ['usb', 'ble', 'nfc'],
            getAuthenticatorData: () => this.authenticatorData.slice().buffer,
        };

        const credential = {
            id: 'mock-credential-id',
            rawId: new Uint8Array([1, 2, 3]).buffer,
            response: credentialResponse,
            type: 'public-key',
            authenticatorAttachment: 'cross-platform',
            getClientExtensionResults: () => ({}),
        };

        return credential as RegistrationCredential;
    }

    async get(challenge: Uint8Array): Promise<AuthenticationCredential> {
        // Manually mangle the digest bytes if changeDigest.
        if (this.changeDigest) {
            challenge = sha256(challenge);
        }

        const clientDataJSON = this.changeClientDataJson
            ? JSON.stringify({
                  type: 'webauthn.create', // Wrong type for clientDataJson.
                  challenge: Buffer.from(challenge).toString('base64'),
                  origin: 'https://www.iota.org',
                  crossOrigin: false,
              })
            : JSON.stringify({
                  type: 'webauthn.get',
                  challenge: Buffer.from(challenge).toString('base64'),
                  origin: 'https://www.iota.org',
                  crossOrigin: false,
              });

        // Sign authenticatorData || sha256(clientDataJSON).
        const dataToSign = new Uint8Array([
            ...this.authenticatorData,
            ...sha256(new TextEncoder().encode(clientDataJSON)),
        ]);

        // Manually mangle the signature if changeSignature.
        const signature = this.changeSignature
            ? secp256r1.sign(sha256(dataToSign), secp256r1.utils.randomPrivateKey())
            : secp256r1.sign(sha256(dataToSign), this.sk);

        const authResponse: AuthenticatorAssertionResponse = {
            authenticatorData: this.changeAuthenticatorData
                ? new Uint8Array([1]).buffer // Change authenticator data
                : this.authenticatorData.slice().buffer,
            clientDataJSON: new TextEncoder().encode(clientDataJSON).slice().buffer,
            signature: signature.toDERRawBytes().slice().buffer,
            userHandle: null,
        };

        const credential = {
            id: 'mock-credential-id',
            rawId: new Uint8Array([1, 2, 3]).buffer,
            type: 'public-key',
            response: authResponse,
            authenticatorAttachment: 'cross-platform',
            getClientExtensionResults: () => ({}),
        };

        return credential as AuthenticationCredential;
    }
}

describe('passkey signer E2E testing', () => {
    it('should retrieve the correct IOTA address', async () => {
        const mockProvider = new MockPasskeySigner();
        const signer = await PasskeyKeypair.getPasskeyInstance(mockProvider);
        const publicKey = signer.getPublicKey();
        expect(publicKey.toIotaAddress()).toEqual(
            '0xaa84bc3c306d5b77238fa2290ecff58957d64c3a167d1205ca67b25610601cb5',
        );
    });

    it('should sign a personal message and verify against pubkey', async () => {
        const sk = secp256r1.utils.randomPrivateKey();
        const pk = secp256r1.getPublicKey(sk);
        const authenticatorData = mockAuthenticatorData;
        const mockProvider = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
        });
        const signer = await PasskeyKeypair.getPasskeyInstance(mockProvider);
        const testMessage = new TextEncoder().encode('Hello world!');
        const { signature } = await signer.signPersonalMessage(testMessage);

        // Verify signature against pubkey.
        const publicKey = signer.getPublicKey();
        const isValid = await publicKey.verifyPersonalMessage(testMessage, signature);
        expect(isValid).toBe(true);

        // Parsed signature as expected.
        const parsed = parseSerializedPasskeySignature(signature);
        expect(parsed.signatureScheme).toEqual('Passkey');
        expect(parsed.publicKey).toEqual(pk);
        expect(new Uint8Array(parsed.authenticatorData!)).toEqual(authenticatorData);

        const messageBytes = bcs.byteVector().serialize(testMessage).toBytes();
        const intentMessage = messageWithIntent('PersonalMessage', messageBytes);
        const digest = blake2b(intentMessage, { dkLen: 32 });
        const clientDataJSON = {
            type: 'webauthn.get',
            challenge: Buffer.from(digest).toString('base64'),
            origin: 'https://www.iota.org',
            crossOrigin: false,
        };
        expect(parsed.clientDataJson).toEqual(JSON.stringify(clientDataJSON));
    });

    it('should sign a transaction and verify against pubkey', async () => {
        const sk = secp256r1.utils.randomPrivateKey();
        const pk = secp256r1.getPublicKey(sk);
        const authenticatorData = mockAuthenticatorData;
        const mockProvider = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
        });
        const signer = await PasskeyKeypair.getPasskeyInstance(mockProvider);

        const messageBytes = fromBase64(
            'AAABACACAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgEBAQABAABnEUWt6SNz7OPa4hXLyCw9tI5Y7rNxhh5DFljH1jLT6QEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAIMqiyOLCIblSqii0TkS8PjMoj3tmA7S24hBMyonz2Op/ZxFFrekjc+zj2uIVy8gsPbSOWO6zcYYeQxZYx9Yy0+noAwAAAAAAAICWmAAAAAAAAA==',
        );
        const intentMessage = messageWithIntent('TransactionData', messageBytes);
        const digest = blake2b(intentMessage, { dkLen: 32 });
        const clientDataJSON = {
            type: 'webauthn.get',
            challenge: Buffer.from(digest).toString('base64'),
            origin: 'https://www.iota.org',
            crossOrigin: false,
        };
        const clientDataJSONString = JSON.stringify(clientDataJSON);

        // Sign the test message.
        const { signature } = await signer.signTransaction(messageBytes);

        // Verify signature against pubkey.
        const publicKey = signer.getPublicKey();
        let isValid = await publicKey.verifyTransaction(messageBytes, signature);
        expect(isValid).toBe(true);

        // Parsed signature as expected.
        const parsed = parseSerializedPasskeySignature(signature);
        expect(parsed.signatureScheme).toEqual('Passkey');
        expect(parsed.publicKey).toEqual(pk);
        expect(new Uint8Array(parsed.authenticatorData!)).toEqual(authenticatorData);
        expect(parsed.clientDataJson).toEqual(clientDataJSONString);

        // Case 1: passkey returns a signature on wrong digest, fails to verify.
        const mockProviderWrongDigest = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
            changeDigest: true,
        });
        const signerWrongDigest = await PasskeyKeypair.getPasskeyInstance(mockProviderWrongDigest);

        const { signature: wrongSignature } = await signerWrongDigest.signTransaction(messageBytes);
        isValid = await publicKey.verifyTransaction(messageBytes, wrongSignature);
        expect(isValid).toBe(false);

        // Case 2: passkey returns wrong type on client data json, fails to verify.
        const mockProviderWrongClientDataJson = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
            changeClientDataJson: true,
        });
        const signerWrongClientDataJson = await PasskeyKeypair.getPasskeyInstance(
            mockProviderWrongClientDataJson,
        );
        const { signature: wrongSignature2 } =
            await signerWrongClientDataJson.signTransaction(intentMessage);
        isValid = await publicKey.verifyTransaction(messageBytes, wrongSignature2);
        expect(isValid).toBe(false);

        // Case 3: passkey returns mismatched authenticator data, fails to verify.
        const mockProviderWrongAuthenticatorData = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
            changeAuthenticatorData: true,
        });
        const signerWrongAuthenticatorData = await PasskeyKeypair.getPasskeyInstance(
            mockProviderWrongAuthenticatorData,
        );
        const { signature: wrongSignature3 } =
            await signerWrongAuthenticatorData.signTransaction(intentMessage);
        isValid = await publicKey.verifyTransaction(messageBytes, wrongSignature3);
        expect(isValid).toBe(false);

        // Case 4: passkey returns a signature from a mismatch secret key, fails to verify.
        const mockProviderWrongSignature = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
            changeSignature: true,
        });
        const signerWrongSignature = await PasskeyKeypair.getPasskeyInstance(
            mockProviderWrongSignature,
        );
        const { signature: wrongSignature4 } =
            await signerWrongSignature.signTransaction(intentMessage);
        isValid = await publicKey.verifyTransaction(messageBytes, wrongSignature4);
        expect(isValid).toBe(false);
    });

    it('should verify a transaction from rust implementation', async () => {
        // generated test vector from `test_passkey_authenticator` in crates/iota-types/src/unit_tests/passkey_authenticator_test.rs

        const sig = fromBase64(
            'BiVYDmenOnqS+thmz5m5SrZnWaKXZLVxgh+rri6LHXs25B0AAAAAgwF7InR5cGUiOiJ3ZWJhdXRobi5nZXQiLCJjaGFsbGVuZ2UiOiJ4NkszMGNvSGlGMF9iczVVVjNzOEVfcGNPNkhMZ0xBb1A3ZE1uU0U5eERNIiwib3JpZ2luIjoiaHR0cHM6Ly93d3cuc3VpLmlvIiwiY3Jvc3NPcmlnaW4iOmZhbHNlfWICAJqKTgco/tSNg4BuVg/f3x+I8NLYN6QqvxHahKNe0PIhBe3EuhfZf8OL4hReW8acT1TVwmPMcnv4SWiAHaX2dAKBYTKkrLK2zLcfP/hD1aiAn/E0L3XLC4epejnzGRhTuA==',
        );

        const txBytes = fromBase64(
            'AAABACACAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgEBAQABAAAt3HtjT61oHCWWztGfhSC2ianNwi6LL2eOLPvZTdJWMgEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAIMqiyOLCIblSqii0TkS8PjMoj3tmA7S24hBMyonz2Op/Ldx7Y0+taBwlls7Rn4UgtompzcIuiy9njiz72U3SVjLoAwAAAAAAAICWmAAAAAAAAA==',
        );
        const parsed = parseSerializedPasskeySignature(sig);
        expect(parsed.signatureScheme).toEqual('Passkey');
        const pubkey = new PasskeyPublicKey(parsed.publicKey);
        const isValid = await pubkey.verifyTransaction(txBytes, sig);
        expect(isValid).toBe(true);
    });

    it('should verify a transaction from a real passkey output', async () => {
        // generated test vector from a real iphone passkey output from browser app
        const sig = fromBase64(
            'BiVJlg3liA6MaHQ0Fw9kdmBbj+SuuaKGMseZXPO6gx2XYx0AAAAAhgF7InR5cGUiOiJ3ZWJhdXRobi5nZXQiLCJjaGFsbGVuZ2UiOiJCUG9qdmNZOWJKRnNqZjJoa0lJX0dndWp0U0dNNlFRVW51Unk0VXhuRVp3Iiwib3JpZ2luIjoiaHR0cDovL2xvY2FsaG9zdDo1MTczIiwiY3Jvc3NPcmlnaW4iOmZhbHNlfWIC6iBZUmJCLqIIGi0pBiEDz+bklP3wAa/dI1d2A6MdJbpY5Ro5io1JsFcAo1TRlr8l4HSOIdmzRTl5Z6eTRtuwwAJOdot913xL+tExR8gymQe0uMmQWyuJgpM5kFcbHcVSmQ==',
        );
        const txBytes = fromBase64(
            'AAACAAgA8gUqAQAAAAAgxq0ACYa04TM9hVVLt1TRC4TycqR68k5AXNm7tAj38/ECAgABAQAAAQECAAABAQD0fqgf0aEoRcV3KtZGQhTJMLfkefcZUZm0FRv/o1CTSwKaP3c5GMVBY9ug8cgXS75q669v+5xLW+4KLFe1o+t4ClQ1AAAAAAAAIFnu374V6vCwfuDoQZ4di/jEzJgdUrOim0IpwVoDGluXq/FTzylxGxwQe0NAhJUaccqdd6Jlngv52iiewuhaHM9TNQAAAAAAACA3nRRejhjqviL59EEWo4Hp+GR1EL2rHlK9RLApwUwBqvR+qB/RoShFxXcq1kZCFMkwt+R59xlRmbQVG/+jUJNL6AMAAAAAAADgbzwAAAAAAAA=',
        );
        const parsed = parseSerializedPasskeySignature(sig);
        expect(parsed.signatureScheme).toEqual('Passkey');
        const pubkey = new PasskeyPublicKey(parsed.publicKey);
        const isValid = await pubkey.verifyTransaction(txBytes, sig);
        expect(isValid).toBe(true);
    });

    it('should sign and recover to an unique public key', async () => {
        const sk = secp256r1.utils.randomPrivateKey();
        const pk = secp256r1.getPublicKey(sk);
        const authenticatorData = new Uint8Array([]);
        const mockProvider = new MockPasskeySigner({
            sk: sk,
            pk: pk,
            authenticatorData: authenticatorData,
        });

        const signer = await PasskeyKeypair.getPasskeyInstance(mockProvider);
        const address = signer.getPublicKey().toIotaAddress();

        const testMessage = new TextEncoder().encode('Hello world!');
        const { credentialId, pubKeys: possiblePks } = await PasskeyKeypair.signAndRecover(
            mockProvider,
            testMessage,
        );

        const testMessage2 = new TextEncoder().encode('Hello world 2!');
        const { pubKeys: possiblePks2 } = await PasskeyKeypair.signAndRecover(
            mockProvider,
            testMessage2,
            [credentialId],
        );

        const commonPk = findCommonPublicKey(possiblePks, possiblePks2);
        const signer2 = new PasskeyKeypair(commonPk.toRawBytes(), mockProvider, credentialId);

        // the address from recovered pk is the same as the one constructed from the same mock provider
        expect(signer2.getPublicKey().toIotaAddress()).toEqual(address);
    });
});
