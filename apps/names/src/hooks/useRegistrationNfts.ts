// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useCurrentAccount } from '@iota/dapp-kit';
import { getNameRegistrationType, getSubnameRegistrationType } from '@iota/iota-names-sdk';
import { IotaParsedData } from '@iota/iota-sdk/client';

import { useIotaNamesClient } from '@/contexts';
import { RegistrationNft } from '@/lib/interfaces/registration.interfaces';

import { useGetAllOwnedObjects } from './useGetAllOwnedObjects';

type RegistrationNftType = 'name' | 'subname';
export function useRegistrationNfts(type: RegistrationNftType = 'name') {
    const { iotaNamesClient } = useIotaNamesClient();
    const account = useCurrentAccount();
    const address = account?.address ?? '';
    const packageId = iotaNamesClient.getPackage('packageId', 'v1');

    const filter = (() => {
        switch (type) {
            case 'name':
                return {
                    StructType: getNameRegistrationType(packageId),
                };
            case 'subname':
                return {
                    StructType: getSubnameRegistrationType(packageId),
                };
        }
    })();

    return useGetAllOwnedObjects(address, filter, {
        select(data) {
            return data.map((nameRecord) => {
                const data = nameRecord?.display?.data;
                const content = nameRecord.content as
                    | Extract<IotaParsedData, { dataType: 'moveObject' }>
                    | undefined
                    | null;
                type NameFields =
                    | undefined
                    | { expiration_timestamp_ms?: string; name_str?: string };
                const fields =
                    type === 'subname'
                        ? (content?.fields as { nft: { fields: NameFields } }).nft.fields
                        : (content?.fields as NameFields);
                return {
                    name: data?.name ?? fields?.name_str ?? '',
                    description: data?.description,
                    imageUrl: data?.image_url,
                    link: data?.link,
                    projectUrl: data?.project_url,
                    isExpired:
                        !!fields?.expiration_timestamp_ms &&
                        new Date(Number(fields?.expiration_timestamp_ms)) < new Date(),
                    expirationDate: new Date(Number(fields?.expiration_timestamp_ms) || 0),
                    id: nameRecord.objectId,
                    isSubname: type === 'subname',
                } satisfies RegistrationNft;
            });
        },
    });
}
