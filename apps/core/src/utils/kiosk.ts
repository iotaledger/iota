// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KIOSK_OWNER_CAP } from '@iota/kiosk';
import { IotaObjectData, IotaObjectResponse, NetworkId, getNetwork } from '@iota/iota-sdk/client';

export const ORIGINBYTE_KIOSK_MODULE =
    '0x95a441d389b07437d00dd07e0b6f05f513d7659b13fd7c5d3923c7d9d847199b::ob_kiosk';

export const ORIGINBYTE_KIOSK_OWNER_TOKEN = `${ORIGINBYTE_KIOSK_MODULE}::OwnerToken`;

export function isKioskOwnerToken(
    network: NetworkId,
    object?: IotaObjectResponse | IotaObjectData | null,
) {
    if (!object) return false;
    const objectData = 'data' in object && object.data ? object.data : (object as IotaObjectData);
    return [
        KIOSK_OWNER_CAP,
        `${getNetwork(network).kiosk?.personalKioskRulePackageId}::personal_kiosk::PersonalKioskCap`,
        ORIGINBYTE_KIOSK_OWNER_TOKEN,
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
    // eslint-disable-next-line @typescript-eslint/no-non-null-asserted-optional-chain
    return fields?.for ?? fields?.kiosk ?? fields?.cap?.fields.for!;
}
