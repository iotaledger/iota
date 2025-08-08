// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { validateIotaName, normalizeIotaName } from '@iota/iota-names-sdk';
import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import * as Yup from 'yup';
import { ValidationError } from 'yup';
import { shouldResolveInputAsName } from './names';
import { ReceiverInputFormValues } from './formTypes';
import type { AddressInputProps } from '../../components';

export { ValidationError };

const MIN_NAME_LENGTH = 3;
const MAX_NAME_LENGTH = 64;

export function createReceivingAddressInputSchema(isNameResolutionEnabled: boolean = false) {
    return {
        to: Yup.string()
            .ensure()
            .trim()
            .required('Recipient address is required')
            .test('is-valid-address', 'Invalid address. Please check again.', (value) => {
                const isNameInput = shouldResolveInputAsName(value);
                if (isNameInput && isNameResolutionEnabled) {
                    return true;
                }

                return isValidIotaAddress(value);
            })
            .test('is-valid-name', 'Invalid name. Please check again.', (value) => {
                const isNameInput = shouldResolveInputAsName(value);
                if (isNameInput && isNameResolutionEnabled) {
                    try {
                        if (value === '' || !value || value === '@') return false;

                        const normalizedName = normalizeIotaName(value, 'dot');

                        const isValid = !validateIotaName(
                            normalizedName,
                            MIN_NAME_LENGTH,
                            MAX_NAME_LENGTH,
                        );

                        return isValid;
                    } catch {
                        return false;
                    }
                }

                return true;
            })
            .test(
                'target-address-is-valid',
                'Invalid name target address.',
                (value, { createError, parent }) => {
                    if (!isNameResolutionEnabled) return true;

                    const { resolvedAddress } = parent as ReceiverInputFormValues;
                    const isNameInput = shouldResolveInputAsName(value);

                    if (!isNameInput) return true;

                    if (resolvedAddress === null) {
                        return createError({
                            message: 'Name does not have a target address',
                        });
                    }

                    if (resolvedAddress === undefined) {
                        return createError({
                            message: 'Name does not exist',
                        });
                    }

                    if (resolvedAddress) {
                        const isValidTargetAddress = isValidIotaAddress(resolvedAddress);
                        if (!isValidTargetAddress) {
                            return createError({
                                message: 'Invalid name target address',
                            });
                        }
                    }

                    return true;
                },
            ),
        resolvedAddress: Yup.string().nullable().optional(),
    } satisfies Partial<Record<keyof ReceiverInputFormValues, Yup.AnySchema>>;
}

export const RECEIVING_ADDRESS_FIELD_IDS = {
    fieldId: 'to',
    resolvedNameFieldId: 'resolvedAddress',
} as const satisfies Partial<Record<keyof AddressInputProps, keyof ReceiverInputFormValues>>;
