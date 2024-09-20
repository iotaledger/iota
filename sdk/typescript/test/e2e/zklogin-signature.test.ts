// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromB64 } from '@iota/bcs';
import { describe, expect, it, test } from 'vitest';

import { IotaGraphQLClient } from '../../src/graphql';
import {
    parseSerializedZkLoginSignature,
    ZkLoginPublicIdentifier,
} from '../../src/zklogin/publickey';
import { getZkLoginSignature, parseZkLoginSignature } from '../../src/zklogin/signature';

const aSignature =
    'BQNNMTE3MDE4NjY4MTI3MDQ1MTcyMTM5MTQ2MTI3OTg2NzQ3NDg2NTc3NTU1NjY1ODY1OTc0MzQ4MTA5NDEyNDA0ODMzNDY3NjkzNjkyNjdNMTQxMjA0Mzg5OTgwNjM2OTIyOTczODYyNDk3NTQyMzA5NzI3MTUxNTM4NzY1Mzc1MzAxNjg4ODM5ODE1MTM1ODQ1ODYxNzIxOTU4NDEBMQMCTDE4Njc0NTQ1MDE2MDI1ODM4NDg4NTI3ODc3ODI3NjE5OTY1NjAxNzAxMTgyNDkyOTk1MDcwMTQ5OTkyMzA4ODY4NTI1NTY5OTgyNzNNMTQ0NjY0MTk2OTg2NzkxMTYzMTM0NzUyMTA2NTQ1NjI5NDkxMjgzNDk1OTcxMDE3NjkyNTY5NTkwMTAwMDMxODg4ODYwOTEwODAzMTACTTExMDcyOTU0NTYyOTI0NTg4NDk2MTQ4NjMyNDc0MDc4NDMyNDA2NjMzMjg4OTQ4MjU2NzE4ODA5NzE0ODYxOTg2MTE5MzAzNTI5NzYwTTE5NzkwNTE2MDEwNzg0OTM1MTAwMTUwNjE0OTg5MDk3OTA4MjMzODk5NzE4NjQ1NTM2MTMwNzI3NzczNzEzNDA3NjExMTYxMzY4MDQ2AgExATADTTEwNDIzMjg5MDUxODUzMDMzOTE1MzgwODEwNTE2MTMwMjA1NzQ3MTgyODY3NTk2NDU3MTM5OTk5MTc2NzE0NDc2NDE1MTQ4Mzc2MzUwTTIxNzg1NzE5Njk1ODQ4MDEzOTA4MDYxNDkyOTg5NzY1Nzc3Nzg4MTQyMjU1ODk3OTg2MzAwMjQxNTYxMjgwMTk2NzQ1MTc0OTM0NDU3ATExeUpwYzNNaU9pSm9kSFJ3Y3pvdkwyRmpZMjkxYm5SekxtZHZiMmRzWlM1amIyMGlMQwFmZXlKaGJHY2lPaUpTVXpJMU5pSXNJbXRwWkNJNkltSTVZV00yTURGa01UTXhabVEwWm1aa05UVTJabVl3TXpKaFlXSXhPRGc0T0RCalpHVXpZamtpTENKMGVYQWlPaUpLVjFRaWZRTTEzMzIyODk3OTMwMTYzMjE4NTMyMjY2NDMwNDA5NTEwMzk0MzE2OTg1Mjc0NzY5MTI1NjY3MjkwNjAwMzIxNTY0MjU5NDY2NTExNzExrgAAAAAAAABhAEp+O5GEAF/5tKNDdWBObNf/1uIrbOJmE+xpnlBD2Vikqhbd0zLrQ2NJyquYXp4KrvWUOl7Hso+OK0eiV97ffwucM8VdtG2hjf/RUGNO5JNUH+D/gHtE9sHe6ZEnxwZL7g==';
const aSignatureInputs = {
    addressSeed: '13322897930163218532266430409510394316985274769125667290600321564259466511711',
    headerBase64:
        'eyJhbGciOiJSUzI1NiIsImtpZCI6ImI5YWM2MDFkMTMxZmQ0ZmZkNTU2ZmYwMzJhYWIxODg4ODBjZGUzYjkiLCJ0eXAiOiJKV1QifQ',
    issBase64Details: {
        indexMod4: 1,
        value: 'yJpc3MiOiJodHRwczovL2FjY291bnRzLmdvb2dsZS5jb20iLC',
    },
    proofPoints: {
        a: [
            '11701866812704517213914612798674748657755566586597434810941240483346769369267',
            '14120438998063692297386249754230972715153876537530168883981513584586172195841',
            '1',
        ],
        b: [
            [
                '1867454501602583848852787782761996560170118249299507014999230886852556998273',
                '14466419698679116313475210654562949128349597101769256959010003188886091080310',
            ],
            [
                '11072954562924588496148632474078432406633288948256718809714861986119303529760',
                '19790516010784935100150614989097908233899718645536130727773713407611161368046',
            ],
            ['1', '0'],
        ],
        c: [
            '10423289051853033915380810516130205747182867596457139999176714476415148376350',
            '21785719695848013908061492989765777788142255897986300241561280196745174934457',
            '1',
        ],
    },
};
const anEphemeralSignature =
    'AEp+O5GEAF/5tKNDdWBObNf/1uIrbOJmE+xpnlBD2Vikqhbd0zLrQ2NJyquYXp4KrvWUOl7Hso+OK0eiV97ffwucM8VdtG2hjf/RUGNO5JNUH+D/gHtE9sHe6ZEnxwZL7g==';

describe('zkLogin signature', () => {
    test('is parsed successfully', () => {
        expect(parseZkLoginSignature(fromB64(aSignature).slice(1))).toMatchObject({
            inputs: aSignatureInputs,
            maxEpoch: '174',
            userSignature: fromB64(anEphemeralSignature),
        });
    });
    test('is serialized successfully', () => {
        expect(
            getZkLoginSignature({
                inputs: aSignatureInputs,
                maxEpoch: '174',
                userSignature: fromB64(anEphemeralSignature),
            }),
        ).toBe(aSignature);
    });
    it(
        'verify personal message with zklogin',
        async () => {
            // this test assumes the localnet epoch is smaller than 3. it will fail if localnet has ran for too long and passed epoch 3.
            // test case generated from `iota keytool zk-login-insecure-sign-personal-message --data "hello" --max-epoch 3`
            let bytes = 'aGVsbG8='; // the base64 encoding of "hello"
            let testSignature =
                'BQNMODIxMjAxNjM1OTAxNDk1MDg0Mjg0MTUyNTc3NTE1NjQ4NzI2MjEzOTk0OTQ3ODkwNjkwMTc5ODI5NjEwMTkyNTI3MTY5MTU2NTE4ME0xNjE1NTM3MDU2ODcyNzI3OTgxODg5MzYwMzc1NDQwNzYxNzM3NzcxNTgwOTA2NTUwMTYyODczNjg4MjcyNTU3NTIzMjgzNDkyMzcyNgExAwJNMTE2MTk3MTE1NjYyNDg1NTk1NzUyNzE0MDEzMTI1NzE2OTg5NTkxMDA2MjM3NjM4NzY0NjM1OTEzNDY1NTY2OTM1NzI5NzQxOTE1MDlMNTIyOTU4MjE1NDQ1MzkxMDM4MzYwMzYzNjEzNTY0NDU5MTc1NTk3NDI1OTQyMDg4NjUxMzYwMTQ2Mjc0OTk5Mzg2NTA2MTkyODU2NAJNMTA5MDE5ODc3NzAyNTI5NzkzOTM2NDM4NDU1MjM1MzQ2NTQ4MjY3MTkyODUzMzA2NzQwNTk3Nzg0Nzg3NzYwODQ2Mjc4NjQyNzg0NzJMMjg0MjQxNTQ4Mjg0NjQyNzg5NzAwNjM2OTIyMDk0NDUyNjUzMzgwNzc3ODIxMzQyOTA5NTQ2NDc1ODc0MTE5NTkxMTU5NjE0MzY4MwIBMQEwA00xODg1NDIyNzM3ODk4ODA1MDA3NTM2NTExNjAxNzEzNTYxOTQ1MzA3NDcyOTcwNzE5OTgyOTA5OTA2OTUwMDk3NzgzNTcwNjY1OTU4OEw0ODU5NzY1MTQ5OTgxMDYyMTIxOTc0Njg3NTYxNzc4NDA2ODU0NzAxNjEyNzk4NTU2NTE3NzQ4OTU1NDA5NzgxMjkxNTA1MDYzNjQxATEod2lhWE56SWpvaWFIUjBjSE02THk5dllYVjBhQzV6ZFdrdWFXOGlMQwI+ZXlKcmFXUWlPaUp6ZFdrdGEyVjVMV2xrSWl3aWRIbHdJam9pU2xkVUlpd2lZV3huSWpvaVVsTXlOVFlpZlFNMjA0MzUzNjY2MDAwMzYzNzU3NDU5MjU5NjM0NTY4NjEzMDc5MjUyMDk0NzAyMTkzMzQwMTg1NjQxNTgxNDg1NDQwMzYxOTYyODQ2NDIDAAAAAAAAAGEA+XrHUDMkMaPswTIFsqIgx3yX6j7IvU1T/1yzw4kjKwjgLL0ZtPQZjc2djX7Q9pFoBdObkSZ6JZ4epiOf05q4BrnG7hYw7z5xEUSmSNsGu7IoT3J0z77lP/zuUDzBpJIA';
            let parsed = parseSerializedZkLoginSignature(testSignature);
            let pk = new ZkLoginPublicIdentifier(parsed.publicKey, {
                client: new IotaGraphQLClient({
                    url: 'http://127.0.0.1:9125',
                }),
            });

            // verifies ok bc max_epoch 3 is within upper bound.
            let res = await pk.verifyPersonalMessage(fromB64(bytes), parsed.signature);
            expect(res).toBe(true);

            // test case generated from `iota keytool zk-login-insecure-sign-personal-message --data "hello" --max-epoch 100`
            // fails to verify bc max_epoch too large.
            let testSignature2 =
                'BQNNMTU3MTEzMjIxMjQyNzE4OTQyODkwNzkwMzcyMTAyNzkxNDU1MzAxMTc4NzgxMDIyOTYzNzQ2Njk5MTE0NzU5MDY3ODYyNDYyNzQ2MTBNMTY2MDg4MjI5MjU0NDI1OTQyMjkxMjY4MDIzMzUyNDE3NDU3NTcwMDc0NjUxMjQ0MTI1OTczMTE2MDM5NzYwMTk2ODk0MzE5ODY5MDYBMQMCTTEzNDQ1MjU4Mzc0Mjk4MTE1MjAzMjEwODM4NzU1Nzk0MDExMTg1NDU0OTgzODgxMTg5OTYwNTQzODc5NjMzMDE5OTQxODEyMDk2MjYzTDE3Njk4NDE1NzUzNDg4NDgzOTEzMjMxMTA3NDMyNDkzMTkyOTAxMTEwNjY0NzE2OTkxMzQwNzY0NjExMzg2OTk5NDg1NDAyODA3MzgCTTE0ODU5NDk0ODMxNjI4MzQyMDEzMTM0NDA4NzAxMTIwNDUxMDI4MDkyMTg4MDAxMTMwOTkxNjkxMjAyNzMyMzA2NzcxODI4NTYxNzU0TTIwMzM1NDE4NjE3NzgyMzU5MTQ2NTg0NzcwNzM0MDcyMzI3NzYwMjAyNDYwMDE2NDY0NjAwNjQzMDA2Nzg5NzAyODg0MzQ1NTkzNjg5AgExATADTTE4Nzk4Mjk5MDAzOTAyMDI3MDcxNTg1ODY5MjY3MzYyOTc5ODUwOTExNzA3Nzk2MzU0NDQyMTY2NzEzOTcyNjQ2NzE2OTQ1OTgyMjM4TTEyMDExNjg0MjA0MDI0NTMxNzY2ODUxMTU0OTAyMzI5Njk4MDIwODQ3NTQ1NDU5NDk2MjA2MDI2NDg5MTE5MzUzODI4NTI2NTE5MzAwATEod2lhWE56SWpvaWFIUjBjSE02THk5dllYVjBhQzV6ZFdrdWFXOGlMQwI+ZXlKcmFXUWlPaUp6ZFdrdGEyVjVMV2xrSWl3aWRIbHdJam9pU2xkVUlpd2lZV3huSWpvaVVsTXlOVFlpZlFNMjA0MzUzNjY2MDAwMzYzNzU3NDU5MjU5NjM0NTY4NjEzMDc5MjUyMDk0NzAyMTkzMzQwMTg1NjQxNTgxNDg1NDQwMzYxOTYyODQ2NDJkAAAAAAAAAGEA+XrHUDMkMaPswTIFsqIgx3yX6j7IvU1T/1yzw4kjKwjgLL0ZtPQZjc2djX7Q9pFoBdObkSZ6JZ4epiOf05q4BrnG7hYw7z5xEUSmSNsGu7IoT3J0z77lP/zuUDzBpJIA';
            let parsed2 = parseSerializedZkLoginSignature(testSignature2);
            let res1 = await pk.verifyPersonalMessage(fromB64(bytes), parsed2.signature);
            expect(res1).toBe(false);
        },
        {
            // this test may be flaky, as it needs to wait for a bit for JWK to be available, but also cannot wait too long that makes max epoch expire.
            retry: 10,
        },
    );

    it(
        'verify transaction data with zklogin',
        async () => {
            // this test assumes the localnet epoch is smaller than 3. it will fail if localnet has ran for too long and passed epoch 3.
            let bytes =
                'AAACACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAxawU0NcBzUEjfOeFhCMcbEO0UZDc8fySmLcBavf7cF8AAAAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEBAQEBAAEAAMDw0OKiyouNDkBV7EghDsd9BV2zU0As2gHXCFumHT1cAc/OLfzdVoEphi8yCEaHgSSrh8nwhU6goYIZ39PLEoYkAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADA8NDiosqLjQ5AVexIIQ7HfQVds1NALNoB1whbph09XAEAAAAAAAAAAQAAAAAAAAAA';
            let testSignature =
                'BQNNMTUzODUzNzAwODM3MTY5NDUxNzk5NTQ4Nzk3ODgwMjEyODE0MDYxODAzODUyMTA3OTY2NjAyNzYwNjMwMTU4MDE0NzE0MzUwNDU5MDVNMTA3NTk1NzUzNjA4MTczNjIzODQ4NzE1MDY1NTkxMzA0NjEwMTAyNzI4MDg5NDY4OTc2NjUwMDg5NjkzNDUzNDkwNzI1NzkyMjE4NzIBMQMCTDE3NTYyNjk4MTk0Nzg2NzkwNTYzMjk1MjAxNTE2ODQ4OTU4NjIxNTQ2Njc3OTY5MDc4NDYxNzU0OTUzNzE3NjE3MTc4MzU1NjIyODFMNzY5MzM5MjIyNDkwNzAwODEzODgzMTMyNDI0NjYxMjA1NjM1MzkyNTU3Njk3NjY4NjIyMzMyMzMwMzE0MzkyNTg2NTg5NDcxNTMzOAJNMTc3MzUxMTQwOTU4MzY3NzY0NDQ0NTc3MTM2MzAwOTQxNzY2Mzc5NTYwMzc3MzQ0MTQ4MDc4OTcyNDk0NTI5NzI5OTQ0OTA1OTc3NTRMOTMxMzYyMzYyMTUwMzM4OTk0MzU4MjQ1Njc5NDkwMjM5NzUyNjc4NjczNjQ1MTQ4MzY0MTAzNzMyMzkzOTg3MzAxNzE0Nzg2NzA0OQIBMQEwA00yMDg3MDcxNTY2MzU5MTYwOTY5MjAzNzk5MzkyNDkwNzMyMjcwMjUwNTM4NzE5MjEyMjI3OTc5MDg0NzgyMzIxOTI4MjQxODc0OTA3M00yMDUyMjg2NTI1NjMyMjY1NTQzOTY2NTI3ODM3OTI1ODQ5NDcyMDQ0MTYzMzcxNzE3MjM3MTYzOTA5Njk2MTM4ODE0MjM0OTUzNDg4NQExKHdpYVhOeklqb2lhSFIwY0hNNkx5OXZZWFYwYUM1emRXa3VhVzhpTEMCPmV5SnJhV1FpT2lKemRXa3RhMlY1TFdsa0lpd2lkSGx3SWpvaVNsZFVJaXdpWVd4bklqb2lVbE15TlRZaWZRTTIwNDM1MzY2NjAwMDM2Mzc1NzQ1OTI1OTYzNDU2ODYxMzA3OTI1MjA5NDcwMjE5MzM0MDE4NTY0MTU4MTQ4NTQ0MDM2MTk2Mjg0NjQyBQAAAAAAAABhAMY6yGE+HfJrftA5rtd/SH4DxhNNXMCfjZNP5XmIBxi46JE9TQeGoArtwbWF3dSI7Vm1DxkGaXh3TT2tGz0yfwi5xu4WMO8+cRFEpkjbBruyKE9ydM++5T/87lA8waSSAA==';
            let parsed = parseSerializedZkLoginSignature(testSignature);
            let pk = new ZkLoginPublicIdentifier(parsed.publicKey, {
                client: new IotaGraphQLClient({
                    url: 'http://127.0.0.1:9125',
                }),
            });

            // verifies ok bc max_epoch 5 is within upper bound.
            let res = await pk.verifyTransaction(fromB64(bytes), parsed.signature);
            expect(res).toBe(true);

            // fails to verify bc max_epoch 100 is too large.
            let bytes2 =
                'AAACACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAmVK6vlS7h4Bk7iSsmbXT73zalN44jN4t6nWMqixOD3MAAAAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEBAQEBAAEAAMDw0OKiyouNDkBV7EghDsd9BV2zU0As2gHXCFumHT1cAReb2i3yG/eC+JHyRIa6pazI7GAPbjy2GlxBjekvTpouAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADA8NDiosqLjQ5AVexIIQ7HfQVds1NALNoB1whbph09XAEAAAAAAAAAAQAAAAAAAAAA';
            let testSignature2 =
                'BQNLNjE1NDU3MTU5NjYwMDM3ODM1MTY2OTE1OTkwMzg1MjQ3NTkzMDg3NTk3NzM4Nzg2MzkxNjE3MTU4MDg1ODIzMTcyNjg4MzkyMjg3TTE0ODc4NTgwNDQ2ODcxNzE3Mzc2ODEwNjM5ODIxNDQxNDk5OTI0MDQxNjMwMzQ5MTExNTAwNzQzOTk2NTY0MzczNTU5NDU2NDg4NDY5ATEDAkwxOTI5MDM5OTU1Mjk3Njg0NzU0NDU5NDc2NzQwMzEyNjMzMDI0NTMwOTk2MjAwMzI3ODUxNzc2NDk5MTUyNjQ3MjUxMjQ1MzA3OTEwTDU4OTY0NzA0NDQyMTkyODA1MDYwNjM5MTI2NTMzODk1ODIyNzIzMDA4NDI0MjkwMDMxNTIxMjk2Njk3OTM1MTc2NTQ1NTczMDMyODcCTDcyOTk2ODY3MjgzOTc3MzQ3MDg3NjYzNDUzMjcwODc2ODc3MTgzOTU5MTQwNzkwMTc0MjIwOTUwNTkzNTg3MTcxMjg3MjA2Njk2OTFNMTAxNzIyNzE1ODY2OTc0MzY4MTU1MTQzOTcyMjkyMTgzMzUxMDg1NDY2NTEzNzI1MTI5MTE2NjI3NDcxNDg5MjU2NDY5OTk0NTQxMjYCATEBMANMNjMwNDA2MTEyMzQ1OTY1NjE5MzQ1ODAzOTA0MzExNTg0OTU2ODgwMDM1Njc5NTU5NTUxNTAwNDAwNTE1ODA3NDkxMTI5MzA3MDI0OEwyMjI2NTQ3MzA3NzY0MzE5NjA1NDgwMzk3NDE4MTM5MTEwODk5NDk2NTQxODM4MzM5OTU0MTkwMDQzOTc0ODQzNTAxNDUxNzc2Mzc5ATEod2lhWE56SWpvaWFIUjBjSE02THk5dllYVjBhQzV6ZFdrdWFXOGlMQwI+ZXlKcmFXUWlPaUp6ZFdrdGEyVjVMV2xrSWl3aWRIbHdJam9pU2xkVUlpd2lZV3huSWpvaVVsTXlOVFlpZlFNMjA0MzUzNjY2MDAwMzYzNzU3NDU5MjU5NjM0NTY4NjEzMDc5MjUyMDk0NzAyMTkzMzQwMTg1NjQxNTgxNDg1NDQwMzYxOTYyODQ2NDJkAAAAAAAAAGEAHvkRO3Mx6v8o57/BfVE/lOyy/XNNIo3I8elULrhPn0f2hENWzhvW+xEfPX0V4de38++4mYi7ctY8XNJmkBPODrnG7hYw7z5xEUSmSNsGu7IoT3J0z77lP/zuUDzBpJIA';
            let parsed2 = parseSerializedZkLoginSignature(testSignature2);
            let res1 = await pk.verifyPersonalMessage(fromB64(bytes2), parsed2.signature);
            expect(res1).toBe(false);
        },
        {
            // this test may be flaky, as it needs to wait for a bit for JWK to be available, but also cannot wait too long that makes max epoch expire.
            retry: 10,
        },
    );
});
