// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';

import type { DataFields, NameObject, NetworkType, IotaNSContract } from './types/objects.js';
import { DEVNET_JSON_FILE, GCS_URL, TESTNET_JSON_FILE } from './utils/constants.js';
import { camelCase, parseObjectDataResponse, parseRegistryResponse } from './utils/parser.js';
import { getAvatar, getOwner } from './utils/queries.js';

export const AVATAR_NOT_OWNED = 'AVATAR_NOT_OWNED';

class IotansClient {
    private iotaClient: IotaClient;
    contractObjects: IotaNSContract | undefined;
    networkType: NetworkType | undefined;

    constructor(
        iotaClient: IotaClient,
        options?: {
            contractObjects?: IotaNSContract;
            networkType?: NetworkType;
        },
    ) {
        if (!iotaClient) {
            throw new Error('IotaClient must be specified.');
        }
        this.iotaClient = iotaClient;
        this.contractObjects = options?.contractObjects;
        this.networkType = options?.networkType;
    }

    async getIotansContractObjects() {
        if ((this.contractObjects as IotaNSContract)?.packageId) return;

        const contractJsonFileUrl =
            GCS_URL + (this.networkType === 'testnet' ? TESTNET_JSON_FILE : DEVNET_JSON_FILE);

        let response;
        try {
            response = await fetch(contractJsonFileUrl);
        } catch (error) {
            throw new Error(`Error getting IotaNS contract objects, ${(error as Error).message}`);
        }

        if (!response?.ok) {
            throw new Error(`Network Error: ${response?.status}`);
        }

        this.contractObjects = await response.json();
    }

    protected async getDynamicFieldObject(
        parentObjectId: string,
        key: unknown,
        type = '0x1::string::String',
    ) {
        const dynamicFieldObject = await this.iotaClient.getDynamicFieldObject({
            parentId: parentObjectId,
            name: {
                type: type,
                value: key,
            },
        });

        if (dynamicFieldObject.error?.code === 'dynamicFieldNotFound') return;

        return dynamicFieldObject;
    }

    protected async getNameData(dataObjectId: string, fields: DataFields[] = []) {
        if (!dataObjectId) return {};

        const { data: dynamicFields } = await this.iotaClient.getDynamicFields({
            parentId: dataObjectId,
        });

        const filteredFields = new Set(fields);
        const filteredDynamicFields = dynamicFields.filter(({ name: { value } }) =>
            filteredFields.has(value as DataFields),
        );

        const data = await Promise.allSettled(
            filteredDynamicFields?.map(({ objectId }) =>
                this.iotaClient
                    .getObject({
                        id: objectId,
                        options: { showContent: true },
                    })
                    .then(parseObjectDataResponse)
                    .then((object) => [camelCase(object.name), object.value]),
            ) ?? [],
        );

        const fulfilledData = data.filter(
            (e) => e.status === 'fulfilled',
        ) as PromiseFulfilledResult<[string, unknown]>[];

        return Object.fromEntries(fulfilledData.map((e) => e.value));
    }

    /**
     * Returns the name object data including:
     *
     * - id: the name object address
     * - owner: the owner address // only if you add the `showOwner` parameter. It includes an extra RPC call.
     * - targetAddress: the linked address
     * - avatar?: the custom avatar id // Only if you add showAvatar parameter. It includes an extra RPC call.
     * - contentHash?: the ipfs cid
     *
     * If the input domain has not been registered, it will return an empty object.
     * If `showAvatar` is included, the owner will be fetched as well.
     *
     * @param key a domain name
     */
    async getNameObject(
        name: string,
        options: { showOwner?: boolean; showAvatar?: boolean } | undefined = {
            showOwner: false,
            showAvatar: false,
        },
    ): Promise<NameObject> {
        const [, domain, topLevelDomain] = name.match(/^(.+)\.([^.]+)$/) || [];
        await this.getIotansContractObjects();

        const registryResponse = await this.getDynamicFieldObject(
            (this.contractObjects as IotaNSContract).registry,
            [topLevelDomain, domain],
            `${this.contractObjects?.packageId}::domain::Domain`,
        );

        const nameObject = parseRegistryResponse(registryResponse);

        // check if we should also query for avatar.
        // we can only query if the object has an avatar set
        // and the query includes avatar.
        const includeAvatar = nameObject.avatar && options?.showAvatar;

        // IF we have showOwner or includeAvatar flag, we fetch the owner &/or avatar,
        // We use Promise.all to do these calls at the same time.
        if (nameObject.nftId && (includeAvatar || options?.showOwner)) {
            const [owner, avatarNft] = await Promise.all([
                getOwner(this.iotaClient, nameObject.nftId),
                includeAvatar
                    ? getAvatar(this.iotaClient, nameObject.avatar)
                    : Promise.resolve(null),
            ]);

            nameObject.owner = owner;

            // Parse avatar NFT, check ownership and fixup the request response.
            if (includeAvatar && avatarNft) {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore-next-line
                if (avatarNft.data?.owner?.AddressOwner === nameObject.owner) {
                    const display = avatarNft.data?.display;
                    nameObject.avatar = display?.data?.image_url || null;
                } else {
                    nameObject.avatar = AVATAR_NOT_OWNED;
                }
            } else {
                delete nameObject.avatar;
            }
        }

        return nameObject;
    }

    /**
     * Returns the linked address of the input domain if the link was set. Otherwise, it will return undefined.
     *
     * @param domain a domain name ends with `.iota`
     */
    async getAddress(domain: string): Promise<string | undefined> {
        const { targetAddress } = await this.getNameObject(domain);

        return targetAddress;
    }

    /**
     * Returns the default name of the input address if it was set. Otherwise, it will return undefined.
     *
     * @param address a Iota address.
     */
    async getName(address: string): Promise<string | undefined> {
        const res = await this.getDynamicFieldObject(
            this.contractObjects?.reverseRegistry ?? '',
            address,
            'address',
        );
        const data = parseObjectDataResponse(res);
        const labels = data?.value?.fields?.labels;

        return Array.isArray(labels) ? labels.reverse()?.join('.') : undefined;
    }
}

export { IotansClient };
