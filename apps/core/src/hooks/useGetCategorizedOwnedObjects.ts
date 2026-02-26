// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState } from 'react';
import { type IotaObjectDataFilter, type IotaObjectResponse } from '@iota/iota-sdk/client';
import { useGetOwnedObjects } from './useGetOwnedObjects';
import { useGetKioskContents } from './useGetKioskContents';
import { useCursorPagination } from './useCursorPagination';
import { hasDisplayData } from '../utils/hasDisplayData';

export enum OwnedObjectCategory {
    Nft = 'nft',
    Name = 'name',
    Kiosk = 'kiosk',
    Other = 'other',
}

interface UseGetCategorizedOwnedObjectsOptions {
    address: string;
    limit: number;
    nameTypes: string[];
}

interface VirtualPagination {
    currentPage: number;
    hasFirst: boolean;
    hasPrev: boolean;
    hasNext: boolean;
    onFirst(): void;
    onPrev(): void;
    onNext(): void;
}

function useVirtualPagination(
    allItems: IotaObjectResponse[],
    limit: number,
    fetchMore: () => void,
    canFetchMore: boolean,
    isFetchingMore: boolean,
): { pageData: IotaObjectResponse[]; pagination: VirtualPagination } {
    const [currentPage, setCurrentPage] = useState(0);

    const totalPages = Math.max(1, Math.ceil(allItems.length / limit));
    const start = currentPage * limit;
    const end = start + limit;
    const pageData = allItems.slice(start, end);

    // Auto-fetch more RPC pages when we don't have enough items for the next virtual page
    useEffect(() => {
        if (canFetchMore && !isFetchingMore && allItems.length <= end) {
            fetchMore();
        }
    }, [canFetchMore, isFetchingMore, allItems.length, end, fetchMore]);

    // Reset to first page when limit changes
    useEffect(() => {
        setCurrentPage(0);
    }, [limit]);

    const hasNext = currentPage < totalPages - 1 || canFetchMore;
    const hasPrev = currentPage > 0;

    const onNext = useCallback(() => {
        const nextPage = currentPage + 1;
        const nextStart = nextPage * limit;

        if (nextStart < allItems.length) {
            setCurrentPage(nextPage);
        } else if (canFetchMore && !isFetchingMore) {
            // Need more data — fetch another RPC page, then advance
            fetchMore();
            setCurrentPage(nextPage);
        }
    }, [currentPage, limit, allItems.length, canFetchMore, isFetchingMore, fetchMore]);

    const onPrev = useCallback(() => {
        setCurrentPage((prev) => Math.max(0, prev - 1));
    }, []);

    const onFirst = useCallback(() => {
        setCurrentPage(0);
    }, []);

    return {
        pageData,
        pagination: {
            currentPage,
            hasFirst: currentPage !== 0,
            hasPrev,
            hasNext: !isFetchingMore && hasNext,
            onFirst,
            onPrev,
            onNext,
        },
    };
}

export function useGetCategorizedOwnedObjects({
    address,
    limit,
    nameTypes,
}: UseGetCategorizedOwnedObjectsOptions) {
    const hasNameTypes = nameTypes.length > 0;

    const namesFilter: IotaObjectDataFilter | undefined = hasNameTypes
        ? {
              MatchAny: nameTypes.map((type) => ({ StructType: type })),
          }
        : undefined;

    const namesQuery = useGetOwnedObjects(hasNameTypes ? address : null, namesFilter, limit);
    const namesPaginated = useCursorPagination(namesQuery);

    const nftsAndOtherFilter: IotaObjectDataFilter = {
        MatchNone: [
            { StructType: '0x2::coin::Coin' },
            ...nameTypes.map((type) => ({ StructType: type })),
        ],
    };

    const nftsAndOtherQuery = useGetOwnedObjects(address, nftsAndOtherFilter, limit);

    const { allNfts, allOther } = useMemo(() => {
        const pages = nftsAndOtherQuery.data?.pages ?? [];
        const nfts: IotaObjectResponse[] = [];
        const other: IotaObjectResponse[] = [];

        for (const page of pages) {
            for (const obj of page.data) {
                if (hasDisplayData(obj)) {
                    nfts.push(obj);
                } else {
                    other.push(obj);
                }
            }
        }

        return { allNfts: nfts, allOther: other };
    }, [nftsAndOtherQuery.data?.pages]);

    const fetchMoreNftsAndOther = useCallback(() => {
        if (nftsAndOtherQuery.hasNextPage && !nftsAndOtherQuery.isFetchingNextPage) {
            nftsAndOtherQuery.fetchNextPage();
        }
    }, [nftsAndOtherQuery]);

    const canFetchMoreNftsAndOther =
        !!nftsAndOtherQuery.hasNextPage && !nftsAndOtherQuery.isFetchingNextPage;
    const isFetchingMoreNftsAndOther = nftsAndOtherQuery.isFetchingNextPage;

    const { pageData: nftPageData, pagination: nftPagination } = useVirtualPagination(
        allNfts,
        limit,
        fetchMoreNftsAndOther,
        canFetchMoreNftsAndOther,
        isFetchingMoreNftsAndOther,
    );

    const { pageData: otherPageData, pagination: otherPagination } = useVirtualPagination(
        allOther,
        limit,
        fetchMoreNftsAndOther,
        canFetchMoreNftsAndOther,
        isFetchingMoreNftsAndOther,
    );

    const { data: kioskData, isFetching: kioskIsFetching } = useGetKioskContents(address);

    const isFetchingNftsAndOther = nftsAndOtherQuery.isFetching;

    const availableCategories = useMemo(() => {
        const categories: OwnedObjectCategory[] = [];
        if (allNfts.length > 0) categories.push(OwnedObjectCategory.Nft);
        if ((namesPaginated.data?.data?.length ?? 0) > 0) categories.push(OwnedObjectCategory.Name);
        if ((kioskData?.list?.length ?? 0) > 0) categories.push(OwnedObjectCategory.Kiosk);
        if (allOther.length > 0) categories.push(OwnedObjectCategory.Other);
        return categories;
    }, [
        allNfts.length,
        namesPaginated.data?.data?.length,
        kioskData?.list?.length,
        allOther.length,
    ]);

    return {
        nft: {
            data: nftPageData,
            isFetching: isFetchingNftsAndOther,
            isError: nftsAndOtherQuery.isError,
            pagination: nftPagination,
        },
        name: {
            data: namesPaginated.data?.data ?? [],
            isFetching: namesPaginated.isFetching,
            isError: namesPaginated.isError,
            pagination: namesPaginated.pagination,
        },
        kiosk: {
            data: kioskData?.list ?? [],
            isFetching: kioskIsFetching,
        },
        other: {
            data: otherPageData,
            isFetching: isFetchingNftsAndOther,
            isError: nftsAndOtherQuery.isError,
            pagination: otherPagination,
        },
        availableCategories,
        isError: nftsAndOtherQuery.isError || namesPaginated.isError,
    };
}
