// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';
import { normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { InactiveValidatorData, ValidatorSchema, DynamicFieldObjectSchema } from '../../types';

export async function getInactiveValidatorsMetadata(
    client: IotaClient,
    validatorObjectId: string,
): Promise<InactiveValidatorData | null> {
    const validatorObject = await client.getObject({
        id: normalizeIotaAddress(validatorObjectId),
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

    const validatorMetadata = metadata.data.fields.value.fields.metadata.fields;
    return {
        imageUrl: validatorMetadata.image_url,
        description: validatorMetadata.description,
        name: validatorMetadata.name,
        projectUrl: validatorMetadata.project_url,
        validatorAddress: validatorMetadata.iota_address,
        validatorPublicKey: toB64(Uint8Array.from(validatorMetadata.protocol_pubkey_bytes)),
        validatorStakingPoolId: validator.data.fields.name,
    };
}
