// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState, useEffect } from 'react';
import { useGetOwnedObjects } from './useGetOwnedObjects';
import { useGetKioskContents } from './useGetKioskContents';
import { useIotaNamesClient } from '../contexts';
import { hasDisplayData } from '../utils/hasDisplayData';
import { getNameRegistrationType, getSubnameRegistrationType } from '@iota/iota-names-sdk';
import type { IotaObjectResponse } from '@iota/iota-sdk/client';
import type { KioskItem } from '@iota/kiosk';

const FETCH_CHUNK_SIZE = 1000;

export enum OwnedObjectCategory {
    Nft = 'nft',
    Name = 'name',
    Kiosk = 'kiosk',
    Other = 'other',
}

interface VirtualPagination {
    currentPage: number;
    hasFirst: boolean;
    hasPrev: boolean;
    hasNext: boolean;
    onFirst: () => void;
    onPrev: () => void;
    onNext: () => void;
}

interface CategoryData<T> {
    data: T[];
    totalItems: number;
    isFetching: boolean;
    isError: boolean;
    pagination: VirtualPagination;
}

interface CategorizedOwnedObjectsResult {
    nft: CategoryData<IotaObjectResponse>;
    name: CategoryData<IotaObjectResponse>;
    kiosk: CategoryData<KioskItem>;
    other: CategoryData<IotaObjectResponse>;
    availableCategories: OwnedObjectCategory[];
    isPending: boolean;
    isAnyError: boolean;
}

function useVirtualPagination<T>(
    items: T[],
    pageSize: number,
    resetKey?: string,
): CategoryData<T> & { setPage: (page: number) => void } {
    const [currentPage, setCurrentPage] = useState(0);
    useEffect(() => {
        setCurrentPage(0);
    }, [pageSize, resetKey]);

    const totalPages = Math.max(1, Math.ceil(items.length / pageSize));

    const safePage = Math.min(currentPage, totalPages - 1);

    const pageData = useMemo(() => {
        const start = safePage * pageSize;
        return items.slice(start, start + pageSize);
    }, [items, safePage, pageSize]);

    const pagination: VirtualPagination = useMemo(
        () => ({
            currentPage: safePage,
            hasFirst: safePage > 0,
            hasPrev: safePage > 0,
            hasNext: safePage < totalPages - 1,
            onFirst: () => setCurrentPage(0),
            onPrev: () => setCurrentPage((p) => Math.max(0, p - 1)),
            onNext: () => setCurrentPage((p) => Math.min(totalPages - 1, p + 1)),
        }),
        [safePage, totalPages],
    );

    return {
        data: pageData,
        totalItems: items.length,
        isFetching: false,
        isError: false,
        pagination,
        setPage: setCurrentPage,
    };
}

export function useGetCategorizedOwnedObjects(
    address: string,
    pageSize: number = 50,
): CategorizedOwnedObjectsResult {
    const ownedObjectsQuery = useGetOwnedObjects(
        address,
        { MatchNone: [{ StructType: '0x2::coin::Coin' }] },
        FETCH_CHUNK_SIZE,
    );

    const {
        data: kioskData,
        isFetching: kioskFetching,
        isError: kioskError,
    } = useGetKioskContents(address);

    const { iotaNamesClient } = useIotaNamesClient();

    const nameTypes = useMemo(() => {
        try {
            const packageId = iotaNamesClient?.getPackage('packageId', 'v1');
            if (!packageId) return [];
            return [getNameRegistrationType(packageId), getSubnameRegistrationType(packageId)];
        } catch {
            // IOTA Names packages are not available on all networks (e.g. localnet)
            return [];
        }
    }, [iotaNamesClient]);

    const allFetchedObjects = useMemo(() => {
        if (!ownedObjectsQuery.data?.pages) return [];
        return ownedObjectsQuery.data.pages.flatMap((page) => page.data ?? []);
    }, [ownedObjectsQuery.data?.pages]);

    const { nftItems, nameItems, otherItems } = useMemo(() => {
        const nft: IotaObjectResponse[] = [];
        const name: IotaObjectResponse[] = [];
        const other: IotaObjectResponse[] = [];

        for (const obj of allFetchedObjects) {
            const objType = obj.data?.type;
            const isIotaName = !!objType && nameTypes.some((t) => objType.includes(t));

            if (isIotaName) {
                name.push(obj);
                continue;
            }

            if (hasDisplayData(obj)) {
                nft.push(obj);
                continue;
            }

            other.push(obj);
        }

        return { nftItems: nft, nameItems: name, otherItems: other };
    }, [allFetchedObjects, nameTypes]);

    const kioskItems = useMemo(() => kioskData?.list ?? [], [kioskData?.list]);

    const nftPagination = useVirtualPagination(nftItems, pageSize, OwnedObjectCategory.Nft);
    const namePagination = useVirtualPagination(nameItems, pageSize, OwnedObjectCategory.Name);
    const otherPagination = useVirtualPagination(otherItems, pageSize, OwnedObjectCategory.Other);
    const kioskPagination = useVirtualPagination(kioskItems, pageSize, OwnedObjectCategory.Kiosk);

    useEffect(() => {
        if (
            ownedObjectsQuery.hasNextPage &&
            !ownedObjectsQuery.isFetchingNextPage &&
            !ownedObjectsQuery.isLoading
        ) {
            ownedObjectsQuery.fetchNextPage();
        }
    }, [
        ownedObjectsQuery.hasNextPage,
        ownedObjectsQuery.isFetchingNextPage,
        ownedObjectsQuery.isLoading,
        ownedObjectsQuery.fetchNextPage,
    ]);

    const availableCategories = useMemo(() => {
        const category: OwnedObjectCategory[] = [];
        if (nftItems.length > 0) category.push(OwnedObjectCategory.Nft);
        if (nameItems.length > 0) category.push(OwnedObjectCategory.Name);
        if (kioskItems.length > 0) category.push(OwnedObjectCategory.Kiosk);
        if (otherItems.length > 0) category.push(OwnedObjectCategory.Other);
        return category;
    }, [nftItems.length, nameItems.length, kioskItems.length, otherItems.length]);

    const ownedObjectsFetched =
        !ownedObjectsQuery.isLoading &&
        !ownedObjectsQuery.isFetchingNextPage &&
        !ownedObjectsQuery.hasNextPage;
    const isPending = !ownedObjectsFetched || kioskFetching;

    const isError = ownedObjectsQuery.isError;

    return {
        nft: {
            ...nftPagination,
            isFetching: !ownedObjectsFetched,
            isError,
        },
        name: {
            ...namePagination,
            isFetching: !ownedObjectsFetched,
            isError,
        },
        kiosk: {
            ...kioskPagination,
            isFetching: kioskFetching,
            isError: kioskError,
        },
        other: {
            ...otherPagination,
            isFetching: !ownedObjectsFetched,
            isError,
        },
        availableCategories: isPending ? [] : availableCategories,
        isPending,
        isAnyError: isError || kioskError,
    };
}
