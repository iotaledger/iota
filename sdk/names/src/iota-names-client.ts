// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';
import { fromBase64, toBase64 } from '@iota/iota-sdk/utils';
import { blake2b } from '@noble/hashes/blake2b';
import { bytesToHex, hexToBytes } from '@noble/hashes/utils';

import { CouponBcs, CouponHouseBcs, DummyFieldBcs, NameBcs } from './bcs.js';
import { ALLOWED_METADATA, packages } from './constants.js';
import { applyCouponsToPrice, validateCoupons } from './coupons.js';
import {
    getConfigType,
    getCoreConfigType,
    getDenyListType,
    getNameType,
    getPricelistConfigType,
    getRegistryKeyType,
    getRenewalPricelistConfigType,
    getSubnamesConfigType,
    isSubname,
    validateYears,
} from './helpers.js';
import type {
    Coupon,
    CouponHouse,
    IotaNamesClientConfig,
    IotaNamesCoreConfig,
    IotaNamesPriceList,
    IotaNamesSubnamesConfig,
    NameRecord,
    PackageInfo,
} from './types.js';
import { isValidIotaName, normalizeIotaName, validateIotaName } from './utils.js';

/// The IotaNamesClient is the main entry point for the IotaNames SDK.
/// It allows you to interact with IOTA-Names.
export class IotaNamesClient {
    graphQlClient: IotaGraphQLClient;
    config: PackageInfo;

    constructor(config: IotaNamesClientConfig) {
        this.graphQlClient = config.graphQlClient;

        if ('network' in config) {
            this.config = packages[config.network as keyof typeof packages];
        } else {
            this.config = config.packageInfo;
        }
    }

    /**
     * Get the corresponding address for the given package.
     * Sometimes new versions might contain new types,
     * so its important to use the package they were created on when reading them in e.g GraphQL.
     *
     * - If the package does not have any versioning then the value is returned directly.
     * - If there are multiple versions available then the version passed as argument will be used to retrieve the package.
     * - If none of above returned a version then the latest will be picked.
     *
     * @argument {string} key - Package key.
     * @argument {string} [version=v1] - Optionally specify your desired version.
     */
    getPackage(key: keyof PackageInfo, version: `v${number}` | 'latest' = 'latest'): string {
        const pkg = this.config[key];

        // This package is not versioned at all
        if (typeof pkg === 'string') return pkg;

        // Select the versioned package, only if its not "latest"
        if (version !== 'latest' && version in pkg && typeof pkg[version] === 'string') {
            return pkg[version];
        }

        // Get the latest available version for that given package
        const fallback = Object.values(pkg).toReversed()[0];
        if (!fallback) throw new Error(`Package ${key} is not set`);

        return fallback;
    }

    /**
     * Returns the core config of IOTA Names.
     */
    async getCoreConfig(): Promise<IotaNamesCoreConfig> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');

        const coreConfigBcsB64 = toBase64(
            DummyFieldBcs.serialize({
                dummy_field: false,
            }).toBytes(),
        );

        const coreConfigResponse: any = await this.graphQlClient.query({
            query: graphql(`
                query getCoreConfig($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        address
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: iotaNamesObjectId,
                name: {
                    type: getConfigType(packageId, getCoreConfigType(packageId)),
                    bcs: coreConfigBcsB64,
                },
            },
        });

        const coreConfig = coreConfigResponse?.data?.owner?.dynamicField?.value?.json;

        if (!coreConfig) {
            throw new Error('Core config not found or is invalid');
        }

        return coreConfig;
    }

    /**
     * Returns the subnames config of IOTA Names.
     */
    async getSubnamesConfig(): Promise<IotaNamesSubnamesConfig> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');
        const subnamesPackageId = this.getPackage('subnamesPackageId', 'v1');

        const subnamesConfigBcsB64 = toBase64(
            DummyFieldBcs.serialize({
                dummy_field: false,
            }).toBytes(),
        );
        const subnamesConfigResponse: any = await this.graphQlClient.query({
            query: graphql(`
                query getSubnameConfig($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        address
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: iotaNamesObjectId,
                name: {
                    type: getConfigType(packageId, getSubnamesConfigType(subnamesPackageId)),
                    bcs: subnamesConfigBcsB64,
                },
            },
        });

        const subnamesConfig = subnamesConfigResponse?.data?.owner?.dynamicField?.value?.json;

        if (!subnamesConfig) {
            throw new Error('Subnames config not found or is invalid');
        }

        return subnamesConfig;
    }

    /**
     * Returns the price list for IOTA names in the base asset.
     */
    // Format:
    // {
    // 	[ 3, 3 ] => 500000000,
    // 	[ 4, 4 ] => 100000000,
    // 	[ 5, 63 ] => 20000000
    // }
    async getPriceList(): Promise<IotaNamesPriceList> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');

        const pricingConfigBcsB64 = toBase64(
            DummyFieldBcs.serialize({
                dummy_field: false,
            }).toBytes(),
        );

        const priceListResponse: any = await this.graphQlClient.query({
            query: graphql(`
                query getPriceList($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        address
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: iotaNamesObjectId,
                name: {
                    type: getConfigType(packageId, getPricelistConfigType(packageId)),
                    bcs: pricingConfigBcsB64,
                },
            },
        });

        const priceList = priceListResponse?.data?.owner?.dynamicField?.value?.json?.pricing;
        const contents = priceList?.contents;

        // Ensure the content exists
        if (!contents) {
            throw new Error('Price list not found or content is invalid');
        }

        const priceMap = new Map();
        for (const entry of contents) {
            const { pos0, pos1 } = entry.key;
            const key = [Number(pos0), Number(pos1)]; // Convert keys to numbers
            const value = Number(entry.value); // Convert value to a number

            priceMap.set(key, value);
        }

        return priceMap;
    }

    /**
     * Returns the renewal price list for IOTA names in the base asset.
     */
    // Format:
    // {
    // 	[ 3, 3 ] => 500000000000,
    // 	[ 4, 4 ] => 250000000000,
    // 	[ 5, 63 ] => 50000000000
    // }
    async getRenewalPriceList(): Promise<IotaNamesPriceList> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');

        const pricingConfigBcsB64 = toBase64(
            DummyFieldBcs.serialize({
                dummy_field: false,
            }).toBytes(),
        );

        const priceListResponse: any = await this.graphQlClient.query({
            query: graphql(`
                query getRenewalPriceList($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        address
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: iotaNamesObjectId,
                name: {
                    type: getConfigType(packageId, getRenewalPricelistConfigType(packageId)),
                    bcs: pricingConfigBcsB64,
                },
            },
        });

        const priceList =
            priceListResponse?.data?.owner?.dynamicField?.value?.json?.config?.pricing;
        const contents = priceList?.contents;

        // Ensure the content exists
        if (!contents) {
            throw new Error('Price list not found or content is invalid');
        }

        const priceMap = new Map();
        for (const entry of contents) {
            const { pos0, pos1 } = entry.key;
            const key = [Number(pos0), Number(pos1)]; // Convert keys to numbers
            const value = Number(entry.value); // Convert value to a number

            priceMap.set(key, value);
        }

        return priceMap;
    }
    async getPublicName(address: string): Promise<string | null> {
        const response: any = await this.graphQlClient.query({
            query: graphql(`
                query resolveNameServiceName($address: IotaAddress!, $nameFormat: NameFormat) {
                    address(address: $address) {
                        publicName: iotaNamesDefaultName(format: $nameFormat)
                    }
                }
            `),
            variables: {
                address,
            },
        });

        const publicName = response?.data?.address?.publicName ?? null;

        return publicName;
    }

    async getDenyListTableIds(): Promise<{
        reservedTableId: string | null;
        blockedTableId: string | null;
    }> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');

        const denyListBcsB64 = toBase64(
            DummyFieldBcs.serialize({
                dummy_field: false,
            }).toBytes(),
        );

        const response: any = await this.graphQlClient.query({
            query: graphql(`
                query getDenyList($address: IotaAddress!, $type: String!, $bcs: Base64!) {
                    owner(address: $address) {
                        dynamicField(name: { type: $type, bcs: $bcs }) {
                            name {
                                type {
                                    repr
                                }
                                json
                            }
                            value {
                                ... on MoveValue {
                                    type {
                                        repr
                                    }
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                address: iotaNamesObjectId,
                type: getRegistryKeyType(packageId, getDenyListType(packageId)),
                bcs: denyListBcsB64,
            },
        });

        const denyList = response?.data?.owner?.dynamicField?.value?.json;
        return {
            reservedTableId: denyList?.reserved?.id ?? null,
            blockedTableId: denyList?.blocked?.id ?? null,
        };
    }

    /**
     * Gets the full reserved/blocked list
     */
    async getRestrictedList(type: 'reserved' | 'blocked'): Promise<string[]> {
        const { reservedTableId, blockedTableId } = await this.getDenyListTableIds();
        if (type === 'reserved' && !reservedTableId) return [];
        if (type === 'blocked' && !blockedTableId) return [];

        const tableId = type === 'reserved' ? reservedTableId : blockedTableId;

        const results: string[] = [];
        let cursor: string | null = null;
        const pageSize = 50;
        let hasMorePages = true;

        while (hasMorePages) {
            const response: any = await this.graphQlClient.query({
                query: graphql(`
                    query getReservedListFromTable(
                        $address: IotaAddress!
                        $first: Int!
                        $after: String
                    ) {
                        owner(address: $address) {
                            dynamicFields(first: $first, after: $after) {
                                pageInfo {
                                    hasNextPage
                                    endCursor
                                }
                                nodes {
                                    name {
                                        json
                                    }
                                }
                            }
                        }
                    }
                `),
                variables: { address: tableId, first: pageSize, after: cursor },
            });

            const df = response?.data?.owner?.dynamicFields;
            const nodes = df?.nodes ?? [];

            for (const n of nodes) {
                const json = n?.name?.json;
                if (typeof json === 'string' && json.length > 0) {
                    results.push(json);
                    continue;
                }
            }
            if (!df?.pageInfo?.hasNextPage) {
                hasMorePages = false;
            } else {
                cursor = df.pageInfo.endCursor;
            }
            cursor = df.pageInfo.endCursor;
        }

        return results;
    }

    async getNameRecord(name: string): Promise<NameRecord | null> {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA name');
        const registryTableId = this.getPackage('registryTableId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');

        const nameBcsB64 = toBase64(
            NameBcs.serialize({
                labels: normalizeIotaName(name, 'dot').split('.').reverse(),
            }).toBytes(),
        );

        const nameRecordResponse: any = await this.graphQlClient.query({
            query: graphql(`
                query getNameRecord($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        address
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    json
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: registryTableId,
                name: {
                    type: getNameType(packageId),
                    bcs: nameBcsB64,
                },
            },
        });

        const nameRecord = nameRecordResponse.data?.owner?.dynamicField?.value?.json;

        // in case the name record is not found, return null
        if (!nameRecord) return null;

        const nameRecordData = nameRecord.data?.contents;

        if (nameRecord.error || !nameRecordData)
            throw new Error('Name record not found. This name is not registered.');

        const data: Record<string, string> = {};

        if (nameRecordData) {
            nameRecordData.forEach((field: any) => {
                if (field.key) {
                    data[field.key as string] = field.value;
                }
            });
        }

        return {
            name,
            nftId: nameRecord?.nft_id,
            targetAddress: nameRecord?.target_address!,
            expirationDate: new Date(Number(nameRecord?.expiration_timestamp_ms)),
            data,
            avatar: data[ALLOWED_METADATA.avatar],
        };
    }

    async getCouponHouse(): Promise<CouponHouse> {
        const iotaNamesObjectId = this.getPackage('iotaNamesObjectId', 'v1');
        const packageId = this.getPackage('packageId', 'v1');
        const couponsPackageId = this.getPackage('couponsPackageId', 'v1');

        const DummyFieldB64 = DummyFieldBcs.serialize({ dummy_field: false }).toBase64();

        const couponHouseResponse = await this.graphQlClient.query<{
            owner: { dynamicField: { value: { bcs: string } } };
        }>({
            query: graphql(`
                query getIotaNamesCouponHouseRegistryKey(
                    $parentId: IotaAddress!
                    $name: DynamicFieldName!
                ) {
                    owner(address: $parentId) {
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    bcs
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: iotaNamesObjectId,
                name: {
                    type: `${packageId}::iota_names::RegistryKey<${couponsPackageId}::coupon_house::CouponHouse>`,
                    bcs: DummyFieldB64,
                },
            },
        });

        const couponsHouseDynamicFieldBcsValue =
            couponHouseResponse?.data?.owner?.dynamicField?.value?.bcs;

        if (!couponsHouseDynamicFieldBcsValue) {
            throw new Error('Coupon house not found or is invalid');
        }

        return CouponHouseBcs.parse(fromBase64(couponsHouseDynamicFieldBcsValue));
    }

    async resolveCoupon(couponCode: string): Promise<Coupon | null> {
        const couponHouse = await this.getCouponHouse();
        const couponsTableId = couponHouse?.coupons?.coupons?.id.id.bytes;

        if (!couponsTableId) {
            throw new Error('Coupons table ID not found in the coupon house');
        }

        const couponCodeHash = bytesToHex(blake2b(couponCode, { dkLen: 32 }));
        const couponCodeBytes = hexToBytes(couponCodeHash);

        const couponCodeB64 = bcs.vector(bcs.u8()).serialize(couponCodeBytes).toBase64();

        const couponResponse = await this.graphQlClient.query<{
            owner: { dynamicField: { value: { bcs: string } } };
        }>({
            query: graphql(`
                query getCouponBcs($parentId: IotaAddress!, $name: DynamicFieldName!) {
                    owner(address: $parentId) {
                        dynamicField(name: $name) {
                            value {
                                ... on MoveValue {
                                    bcs
                                }
                            }
                        }
                    }
                }
            `),
            variables: {
                parentId: couponsTableId,
                name: {
                    type: 'vector<u8>',
                    bcs: couponCodeB64,
                },
            },
        });

        const couponBcsBase64 = couponResponse?.data?.owner?.dynamicField?.value?.bcs;

        if (!couponBcsBase64) {
            return null;
        }

        const couponData = CouponBcs.parse(fromBase64(couponBcsBase64));

        return { ...couponData, couponCode };
    }

    /**
     * Calculates the registration or renewal price for an SLN (Second Level Name).
     * It expects a name, the number of years and a `IotaNamesPriceList` object,
     * as returned from `iotaNamesClient.getPriceList()` function, or `iotaNames.getRenewalPriceList()` function.
     *
     * It throws an error:
     * 1. if the name is a subname
     * 2. if the name is not a valid IOTA name
     * 3. if the years are not between 1 and 5
     */
    async calculatePrice({
        name,
        years,
        isRegistration = true,
    }: {
        name: string;
        years: number;
        isRegistration?: boolean;
    }) {
        if (!isValidIotaName(name)) {
            throw new Error('Invalid IOTA names');
        }
        validateYears(years);

        if (isSubname(name)) {
            throw new Error('Subnames do not have a registration fee');
        }

        const length = normalizeIotaName(name, 'dot').split('.')[0].length;
        const priceList = await this.getPriceList();
        const renewalPriceList = await this.getRenewalPriceList();
        let yearsRemain = years;
        let price = 0;

        if (isRegistration) {
            for (const [[minLength, maxLength], pricePerYear] of priceList.entries()) {
                if (length >= minLength && length <= maxLength) {
                    price += pricePerYear; // Registration is always 1 year
                    yearsRemain -= 1;
                    break;
                }
            }
        }

        for (const [[minLength, maxLength], pricePerYear] of renewalPriceList.entries()) {
            if (length >= minLength && length <= maxLength) {
                price += yearsRemain * pricePerYear;
                break;
            }
        }

        return price;
    }

    async calculateDiscountedPrice({
        coupons,
        name,
        years,
        isRegistration = true,
        address,
    }: {
        coupons: Coupon[] | string[];
        name: string;
        years: number;
        isRegistration?: boolean;
        address?: string;
    }) {
        if (coupons.every((coupon) => typeof coupon === 'string')) {
            const couponPromises = (coupons as string[]).map(async (couponCode) => {
                const coupon = await this.resolveCoupon(couponCode);
                if (!coupon) {
                    throw new Error(`Coupon not found: ${couponCode}`);
                }

                return coupon;
            });

            coupons = (await Promise.all(couponPromises)) as Coupon[];
        }

        const normalizedName = normalizeIotaName(name, 'dot');

        validateIotaName(normalizedName);

        const nameParts = normalizedName.split('.');
        const firstNamePart = nameParts[0];

        validateCoupons(coupons, years, firstNamePart.length, address);

        const standardPrice = await this.calculatePrice({
            name,
            years,
            isRegistration,
        });

        return applyCouponsToPrice(coupons, standardPrice);
    }
}
