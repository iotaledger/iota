// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Yup from 'yup';
import { createReceivingAddressInputSchema, ReceiverInputFormValues } from '../validation';
import { ValidationError } from 'yup';

export function createNftSendValidationSchema(
    senderAddress: string,
    objectId: string,
    isNameResolutionEnabled: boolean = false,
) {
    const baseSchema = createReceivingAddressInputSchema(isNameResolutionEnabled);

    return Yup.object({
        ...baseSchema,
        to: baseSchema.to
            .test('sender-address', 'NFT is owned by this address', (value, { parent }) => {
                const { resolvedAddress } = parent as ReceiverInputFormValues;

                if (resolvedAddress) {
                    return senderAddress !== resolvedAddress;
                }

                return senderAddress !== value;
            })
            .test(
                'nft-sender-address',
                'NFT address must be different from receiver address',
                (value) => objectId !== value,
            ),
    });
}

export { ValidationError };
