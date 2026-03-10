// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, IotaObjectData, IotaObjectResponse, NetworkId } from '@iota/iota-sdk/client';
import { KIOSK_OWNER_CAP } from '@iota/kiosk';

export function isKioskOwnerToken(
    network: NetworkId,
    object?: IotaObjectResponse | IotaObjectData | null,
) {
    if (!object) return false;
    const objectData = 'data' in object && object.data ? object.data : (object as IotaObjectData);
    return [
        KIOSK_OWNER_CAP,
        `${getNetwork(network).kiosk?.personalKioskRulePackageId}::personal_kiosk::PersonalKioskCap`,
    ].includes(objectData?.type ?? '');
}

export function getKioskIdFromOwnerCap(object: IotaObjectResponse | IotaObjectData) {
    const objectData = 'data' in object && object.data ? object.data : (object as IotaObjectData);
    const fields =
        objectData.content?.dataType === 'moveObject'
            ? (objectData.content.fields as {
                  for?: string;
                  kiosk?: string;
                  cap?: { fields: { for: string } };
              })
            : null;
    return fields?.for ?? fields?.kiosk ?? fields?.cap?.fields.for;
}
