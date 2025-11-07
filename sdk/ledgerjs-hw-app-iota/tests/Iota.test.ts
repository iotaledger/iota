// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { openTransportReplayer, RecordStore } from '@ledgerhq/hw-transport-mocker';
import { describe, expect, it } from 'vitest';
import SpeculosHttpTransport from '@ledgerhq/hw-transport-node-speculos-http';
import Axios from 'axios';
import Iota from '../src/Iota';

const API_PORT: number = 5000;
const SPECULOS_BASE_URL: string = `http://127.0.0.1:${API_PORT}`;
const SPECULOS_HTTP_TRANSPORT_OPTS = {
    apiPort: API_PORT.toString(),
};
const TEST_TIMEOUT_MS = 40000;
const WAIT_TIME_MS = 7000;
const PRESS_TIME_MS = 500;
// Before running the tests, pull the speculos Docker image and download the ledgernano binary.
// Then start the speculos simulator and run the tests.
//
// Pull speculos docker image:
// docker pull ghcr.io/ledgerhq/speculos
//
// Download ledgernano binary:
// curl -L -o nanox.tar.gz https://github.com/iotaledger/ledger-app-iota/releases/download/ledger-app-iota-v1.0.0/nanox.tar.gz
// tar -xvf nanox.tar.gz
// mv nanox/iota sdk/ledgerjs-hw-app-iota/tests/iota
//
// Start speculos simulator:
// docker run --rm -d -p 5000:5000 -v $(pwd)/sdk/ledgerjs-hw-app-iota/tests:/app ghcr.io/ledgerhq/speculos --api-port 5000 --display headless /app/iota
//
// Run tests:
// pnpm --filter @iota/ledgerjs-hw-app-iota test
describe.sequential('Test ledgerjs-hw-app-iota', () => {
    it('Iota init', async () => {
        const transport = await openTransportReplayer(RecordStore.fromString(''));
        const pkt = new Iota(transport);
        expect(pkt).not.toBe(undefined);
    });

    it('Test address generation', async () => {
        const transport = await SpeculosHttpTransport.open(SPECULOS_HTTP_TRANSPORT_OPTS);
        const ledgerClient = new Iota(transport);

        const { publicKey } = await ledgerClient.getPublicKey(`m/44'/4218'/0'/0'/0'`);

        // Default speculos mnemonic: glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin
        expect(Buffer.from(publicKey).toString('hex')).toBe(
            'f0a9c612b7e69f1a114aa9189c1f32997d395d09d183368ddfd6d5dc49e34647',
        );
        await Axios.delete(SPECULOS_BASE_URL + '/events');
    });

    it('Test address generation with display', { timeout: TEST_TIMEOUT_MS }, async () => {
        const transport = await SpeculosHttpTransport.open(SPECULOS_HTTP_TRANSPORT_OPTS);
        const ledgerClient = new Iota(transport);

        let addressReceived = false;
        ledgerClient
            .getPublicKey(`m/44'/4218'/0'/0'/0'`, true)
            .then(({ publicKey }) => {
                // Default speculos mnemonic: glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin
                expect(Buffer.from(publicKey).toString('hex')).toBe(
                    'f0a9c612b7e69f1a114aa9189c1f32997d395d09d183368ddfd6d5dc49e34647',
                );
                addressReceived = true;
            })
            .catch((err) => {
                throw new Error(err);
            });
        // Send requests to approve the shown address
        for (let i = 0; i < 3; i++) {
            await Axios.post(SPECULOS_BASE_URL + '/button/right', { action: 'press-and-release' });
            await new Promise((r) => setInterval(r, PRESS_TIME_MS));
        }
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        await new Promise((r) => setInterval(r, WAIT_TIME_MS));
        if (!addressReceived) {
            throw new Error(`Didn't receive address in time`);
        }
        await Axios.delete(SPECULOS_BASE_URL + '/events');
    });

    it('Test signing', { timeout: TEST_TIMEOUT_MS }, async () => {
        const transport = await SpeculosHttpTransport.open(SPECULOS_HTTP_TRANSPORT_OPTS);
        const ledgerClient = new Iota(transport);
        let signatureReceived = false;
        ledgerClient
            .signTransaction(
                `m/44'/4218'/0'/0'/0'`,
                Uint8Array.from(
                    Buffer.from(
                        '0000000000020008e803000000000000002021c22f952c8742b3156dfca5fc8278bd3ba7b209c81e26c4f44a9944259b03b50202000101000001010200000101006fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e2101cad8ac9d85be1fcb1ec3f5870a50004549f4f892856b70499ed1654201c4399984470b000000000020ec2f226e6647a523608dc52ccb9976720c51d60ebfeadc524ee870cdfd1f6b8c6fb21feead027da4873295affd6c4f3618fe176fa2fbf3e7b5ef1d9463b31e21e803000000000000404b4c000000000000',
                        'hex',
                    ),
                ),
            )
            .then(({ signature }) => {
                expect(Buffer.from(signature).toString('hex')).toBe(
                    '9aaa0b45f0aeef61b055fe5c76a9184e6d6b7b361ff77387bd9c43873b07e349300ab7dce9602bf59c287600cdb9b4ade00257c683de65b51f18aee4ed402e0c',
                );
                signatureReceived = true;
            })
            .catch((err) => {
                throw new Error(err);
            });
        await new Promise((resolve) => setTimeout(resolve, WAIT_TIME_MS));
        // Send requests to approve the tx
        for (let i = 0; i < 7; i++) {
            await Axios.post(SPECULOS_BASE_URL + '/button/right', { action: 'press-and-release' });
            await new Promise((r) => setInterval(r, PRESS_TIME_MS));
        }
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        await new Promise((resolve) => setTimeout(resolve, WAIT_TIME_MS));
        if (!signatureReceived) {
            throw new Error(`Didn't receive signature in time`);
        }
        await Axios.delete(SPECULOS_BASE_URL + '/events');
    });

    it('Test blind signing', { timeout: TEST_TIMEOUT_MS }, async () => {
        // Enable blind signing
        await Axios.post(SPECULOS_BASE_URL + '/button/right', { action: 'press-and-release' });
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        await Axios.post(SPECULOS_BASE_URL + '/button/right', { action: 'press-and-release' });
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });

        const transport = await SpeculosHttpTransport.open(SPECULOS_HTTP_TRANSPORT_OPTS);
        const ledgerClient = new Iota(transport);
        let signatureReceived = false;
        ledgerClient
            .signTransaction(
                `m/44'/4218'/0'/0'/0'`,
                Uint8Array.from(
                    Buffer.from(
                        '0000000000000000000000000000000000000000000000000000000000000000',
                        'hex',
                    ),
                ),
            )
            .then(({ signature }) => {
                expect(Buffer.from(signature).toString('hex')).toBe(
                    'c05235724452fd33c4df3558117f47ca807a9bd70750022d414f96790d3ec1c7e08a0c12b52972edd68f535f040357c20ea226d6d06f09e670f008916395c003',
                );
                signatureReceived = true;
            })
            .catch((err) => {
                throw new Error(err);
            });
        await new Promise((resolve) => setTimeout(resolve, WAIT_TIME_MS));
        // Send requests to approve the tx
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        for (let i = 0; i < 3; i++) {
            await Axios.post(SPECULOS_BASE_URL + '/button/right', { action: 'press-and-release' });
            await new Promise((r) => setInterval(r, PRESS_TIME_MS));
        }
        await Axios.post(SPECULOS_BASE_URL + '/button/both', { action: 'press-and-release' });
        await new Promise((resolve) => setTimeout(resolve, WAIT_TIME_MS));
        if (!signatureReceived) {
            throw new Error(`Didn't receive signature in time`);
        }
        await Axios.delete(SPECULOS_BASE_URL + '/events');
    });
});
