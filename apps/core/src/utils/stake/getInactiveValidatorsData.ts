// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';
import { normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { InactiveValidatorData } from '../../types';
import { ValidatorSchema, DynamicFieldObjectSchema } from '../../constants';

// Function to get inactive validator data
// It fetches the validator object and its dynamic fields to extract metadata
export async function getInactiveValidatorsData(
    client: IotaClient,
    objectId: string,
): Promise<InactiveValidatorData | null> {
    const validatorObject = await client.getObject({
        id: normalizeIotaAddress(objectId),
        options: {
            showContent: true,
        },
    });

    const validator = ValidatorSchema.safeParse(validatorObject.data?.content);
    const validatorFieldId = validator.data?.fields.value.fields.inner.fields.id.id;
    if (!validatorFieldId) {
        return null;
    }
    const dynamicFields = await client.getDynamicFields({
        parentId: normalizeIotaAddress(validatorFieldId),
        cursor: null,
        limit: 10,
    });
    const dfObjectId = dynamicFields.data?.[0]?.objectId;
    const dfObject = await client.getObject({
        id: normalizeIotaAddress(dfObjectId),
        options: {
            showContent: true,
        },
    });
    const metadata = DynamicFieldObjectSchema.safeParse(dfObject.data?.content);
    if (!metadata.data || !validator.data) {
        return null;
    }
    return {
        imageUrl: metadata.data.fields.value.fields.metadata.fields.image_url,
        description: metadata.data.fields.value.fields.metadata.fields.description,
        name: metadata.data.fields.value.fields.metadata.fields.name,
        projectUrl: metadata.data.fields.value.fields.metadata.fields.project_url,
        validatorAddress: metadata.data.fields.value.fields.metadata.fields.iota_address,
        validatorPublicKey: toB64(
            Uint8Array.from(
                metadata.data.fields.value.fields.metadata.fields.protocol_pubkey_bytes,
            ),
        ),
        validatorStakingPoolId: validator.data.fields.name,
    };
}
