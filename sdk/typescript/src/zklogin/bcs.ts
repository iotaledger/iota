// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { InferBcsInput } from '@iota/bcs';
import { bcs } from '@iota/bcs';

export const zkLoginSignature = bcs.struct('ZkLoginSignature', {
    inputs: bcs.struct('ZkLoginSignatureInputs', {
        proofPoints: bcs.struct('ZkLoginSignatureInputsProofPoints', {
            a: bcs.vector(bcs.string()),
            b: bcs.vector(bcs.vector(bcs.string())),
            c: bcs.vector(bcs.string()),
        }),
        issBase64Details: bcs.struct('ZkLoginSignatureInputsClaim', {
            value: bcs.string(),
            indexMod4: bcs.u8(),
        }),
        headerBase64: bcs.string(),
        addressSeed: bcs.string(),
    }),
    maxEpoch: bcs.u64(),
    userSignature: bcs.vector(bcs.u8()),
});

export type ZkLoginSignature = InferBcsInput<typeof zkLoginSignature>;
export type ZkLoginSignatureInputs = ZkLoginSignature['inputs'];
