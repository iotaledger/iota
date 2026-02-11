// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    PaginatedObjectsResponse,
    PaginationArguments,
    IotaClient,
    IotaObjectData,
    IotaObjectResponse,
} from '@iota/iota-sdk/client';
import { isValidIotaAddress } from '@iota/iota-sdk/utils';

import type {
    FetchKioskOptions,
    KioskExtension,
    KioskListing,
    OwnedKiosks,
    PagedKioskData,
} from '../types/index.js';
import { KIOSK_OWNER_CAP } from '../types/index.js';
import {
    attachListingsAndPrices,
    attachLockedItems,
    attachObjects,
    extractKioskData,
    getAllDynamicFields,
    getAllObjects,
    getKioskObject,
} from '../utils.js';

const DEFAULT_PAGE_SIZE = 50;
const PERSONAL_KIOSKS_CURSOR = 'personal';
const OWNED_KIOSKS_CURSOR = 'owned';

export async function fetchKiosk(
    client: IotaClient,
    kioskId: string,
    pagination: PaginationArguments<string>,
    options: FetchKioskOptions,
): Promise<PagedKioskData> {
    // TODO: Replace the `getAllDynamicFields` with a paginated
    // response, once we have better RPC support for
    // type filtering & batch fetching.
    // This can't work with pagination currently.
    const data = await getAllDynamicFields(client, kioskId, pagination);

    const listings: KioskListing[] = [];
    const lockedItemIds: string[] = [];

    // extracted kiosk data.
    const kioskData = extractKioskData(data, listings, lockedItemIds, kioskId);

    // split the fetching in two queries as we are most likely passing different options for each kind.
    // For items, we usually seek the Display.
    // For listings we usually seek the DF value (price) / exclusivity.
    const [kiosk, listingObjects, items] = await Promise.all([
        options.withKioskFields ? getKioskObject(client, kioskId) : Promise.resolve(undefined),
        options.withListingPrices
            ? getAllObjects(client, kioskData.listingIds, {
                  showContent: true,
              })
            : Promise.resolve([]),
        options.withObjects
            ? getAllObjects(
                  client,
                  kioskData.itemIds,
                  options.objectOptions || { showDisplay: true },
              )
            : Promise.resolve([]),
    ]);

    if (options.withKioskFields) kioskData.kiosk = kiosk;
    // attach items listings. IF we have `options.withListingPrices === true`, it will also attach the prices.
    attachListingsAndPrices(kioskData, listings, listingObjects);
    // add `locked` status to items that are locked.
    attachLockedItems(kioskData, lockedItemIds);

    // Attach the objects for the queried items.
    attachObjects(
        kioskData,
        items.filter((x) => !!x.data).map((x) => x.data!),
    );

    return {
        data: kioskData,
        nextCursor: null,
        hasNextPage: false,
    };
}

/**
 * A function to fetch all the user's kiosk Caps
 * And a list of the kiosk address ids.
 * Returns a list of `kioskOwnerCapIds` and `kioskIds`.
 * Extra options allow pagination.
 */
export async function getOwnedKiosks(
    client: IotaClient,
    address: string,
    options?: {
        pagination?: PaginationArguments<string>;
        personalKioskType: string;
    },
): Promise<OwnedKiosks> {
    // TODO: this should throw an error, but I am not going to change it right now incase it breaks existing code.
    if (!isValidIotaAddress(address)) {
        return {
            nextCursor: null,
            hasNextPage: false,
            kioskOwnerCaps: [],
            kioskIds: [],
        };
    }

    const limit = options?.pagination?.limit ?? DEFAULT_PAGE_SIZE;
    const [cursorType, cursor] = options?.pagination?.cursor?.split(':') ?? [
        PERSONAL_KIOSKS_CURSOR,
        null,
    ];

    if (options?.personalKioskType && cursorType === PERSONAL_KIOSKS_CURSOR) {
        const personalKioskResponse = formatOwnedKioskResponse(
            await client.getOwnedObjects({
                owner: address,
                filter: {
                    StructType: options.personalKioskType,
                },
                options: {
                    showContent: true,
                    showType: true,
                },
                cursor,
                limit,
            }),
            PERSONAL_KIOSKS_CURSOR,
        );

        if (personalKioskResponse.hasNextPage) {
            return personalKioskResponse;
        }

        const remainingLimit = limit - personalKioskResponse.kioskOwnerCaps.length;

        // If we have all personal kiosks, but don't have space for the owned kiosks
        // we need to start loading owned kiosks for the next page, but don't have a real cursor
        if (remainingLimit < 1) {
            return {
                nextCursor: `${OWNED_KIOSKS_CURSOR}:`,
                hasNextPage: true,
                kioskOwnerCaps: personalKioskResponse.kioskOwnerCaps,
                kioskIds: personalKioskResponse.kioskIds,
            };
        }

        const ownedKiosksResponse = formatOwnedKioskResponse(
            await client.getOwnedObjects({
                owner: address,
                filter: {
                    StructType: KIOSK_OWNER_CAP,
                },
                options: {
                    showContent: true,
                    showType: true,
                },
                limit: remainingLimit,
            }),
            OWNED_KIOSKS_CURSOR,
        );

        return {
            nextCursor: ownedKiosksResponse.nextCursor,
            hasNextPage: ownedKiosksResponse.hasNextPage,
            kioskOwnerCaps: [
                ...personalKioskResponse.kioskOwnerCaps,
                ...ownedKiosksResponse.kioskOwnerCaps,
            ],
            kioskIds: [...personalKioskResponse.kioskIds, ...ownedKiosksResponse.kioskIds],
        };
    }

    return formatOwnedKioskResponse(
        await client.getOwnedObjects({
            owner: address,
            filter: {
                StructType: KIOSK_OWNER_CAP,
            },
            options: {
                showContent: true,
                showType: true,
            },
            // cursor might be an empty string if the number of personal kiosks was a multiple of the limit.
            cursor: cursor ? cursor : null,
            limit,
        }),
        OWNED_KIOSKS_CURSOR,
    );
}

function formatOwnedKioskResponse(
    response: PaginatedObjectsResponse,
    cursorType: string,
): OwnedKiosks {
    const { data, hasNextPage, nextCursor } = response;

    // get kioskIds from the OwnerCaps.
    const kioskIdList = data?.map((x: IotaObjectResponse) => {
        const fields =
            x.data?.content?.dataType === 'moveObject'
                ? (x.data.content.fields as unknown as
                      | {
                            cap: { fields: { for: string } };
                            for?: never;
                        }
                      | {
                            cap?: never;
                            for: string;
                        })
                : null;
        return fields?.cap ? fields?.cap?.fields?.for : (fields?.for as string);
    });

    // clean up data that might have an error in them.
    // only return valid objects.
    const filteredData = data.filter((x) => 'data' in x).map((x) => x.data) as IotaObjectData[];

    return {
        nextCursor: nextCursor ? `${cursorType}:${nextCursor}` : nextCursor,
        hasNextPage,
        kioskOwnerCaps: filteredData.map((x, idx) => ({
            isPersonal: x.type !== KIOSK_OWNER_CAP,
            digest: x.digest,
            version: x.version,
            objectId: x.objectId,
            kioskId: kioskIdList[idx],
        })),
        kioskIds: kioskIdList,
    };
}

// Get a kiosk extension data for a given kioskId and extensionType.
export async function fetchKioskExtension(
    client: IotaClient,
    kioskId: string,
    extensionType: string,
): Promise<KioskExtension | null> {
    const extension = await client.getDynamicFieldObjectV2({
        parentObjectId: kioskId,
        name: {
            type: `0x2::kiosk_extension::ExtensionKey<${extensionType}>`,
            value: {
                dummy_field: false,
            },
        },
        options: {
            showContent: true,
        },
    });

    if (!extension.data) return null;

    const fields = (extension?.data?.content as { fields: { [k: string]: any } })?.fields?.value
        ?.fields;

    return {
        objectId: extension.data.objectId,
        type: extensionType,
        isEnabled: fields?.is_enabled,
        permissions: fields?.permissions,
        storageId: fields?.storage?.fields?.id?.id,
        storageSize: fields?.storage?.fields?.size,
    };
}
