// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetKioskContents, useGetOwnedObjects, useLocalStorage } from '@iota/core';
import {
    Button,
    ButtonSize,
    Divider,
    DividerType,
    Title,
    TitleSize,
    ButtonType,
    SegmentedButtonType,
    ButtonSegmentType,
    ButtonSegment,
    SegmentedButton,
} from '@iota/apps-ui-kit';
import { ListViewLarge, ListViewMedium, ListViewSmall } from '@iota/ui-icons';
import { Text } from '@iota/ui';
import clsx from 'clsx';
import { useEffect, useMemo, useState } from 'react';

import { ListView, SmallThumbnailsView, ThumbnailsView } from '~/components';
import { ObjectViewMode } from '~/lib/enums';
import { Pagination, useCursorPagination } from '~/components/ui';

const PAGE_SIZES = [10, 20, 30, 40, 50];
const SHOW_PAGINATION_MAX_ITEMS = 9;
const OWNED_OBJECTS_LOCAL_STORAGE_VIEW_MODE = 'owned-objects/viewMode';
const OWNED_OBJECTS_LOCAL_STORAGE_FILTER = 'owned-objects/filter';

interface ItemsRangeFromCurrentPage {
    start: number;
    end: number;
}

enum FilterValue {
    All = 'all',
    Kiosks = 'kiosks',
}

const FILTER_OPTIONS = [
    { label: 'NFTS', value: FilterValue.All },
    { label: 'KIOSKS', value: FilterValue.Kiosks },
];

const VIEW_MODES = [
    { icon: <ListViewSmall />, value: ObjectViewMode.List },
    { icon: <ListViewMedium />, value: ObjectViewMode.SmallThumbnail },
    { icon: <ListViewLarge />, value: ObjectViewMode.Thumbnail },
];

function getItemsRangeFromCurrentPage(
    currentPage: number,
    itemsPerPage: number,
): ItemsRangeFromCurrentPage {
    const start = currentPage * itemsPerPage + 1;
    const end = start + itemsPerPage - 1;
    return { start, end };
}

function getShowPagination(
    filter: string | undefined,
    itemsLength: number,
    currentPage: number,
    isFetching: boolean,
): boolean {
    if (filter === FilterValue.Kiosks) {
        return false;
    }

    if (isFetching) {
        return true;
    }

    return currentPage !== 0 || itemsLength > SHOW_PAGINATION_MAX_ITEMS;
}

interface OwnedObjectsProps {
    id: string;
}
export function OwnedObjects({ id }: OwnedObjectsProps): JSX.Element {
    const [limit, setLimit] = useState(50);
    const [filter, setFilter] = useLocalStorage<string | undefined>(
        OWNED_OBJECTS_LOCAL_STORAGE_FILTER,
        undefined,
    );

    const [viewMode, setViewMode] = useLocalStorage(
        OWNED_OBJECTS_LOCAL_STORAGE_VIEW_MODE,
        ObjectViewMode.Thumbnail,
    );

    const ownedObjects = useGetOwnedObjects(
        id,
        {
            MatchNone: [{ StructType: '0x2::coin::Coin' }],
        },
        limit,
    );
    const { data: kioskData, isFetching: kioskDataFetching } = useGetKioskContents(id);

    const { data, isError, isFetching, pagination } = useCursorPagination(ownedObjects);

    const isPending = filter === FilterValue.All ? isFetching : kioskDataFetching;

    useEffect(() => {
        if (!isPending) {
            setFilter(
                kioskData?.list?.length && filter === FilterValue.Kiosks
                    ? FilterValue.Kiosks
                    : FilterValue.All,
            );
        }
    }, [filter, isPending, kioskData?.list?.length, setFilter]);

    const filteredData = useMemo(
        () => (filter === FilterValue.All ? data?.data : kioskData?.list),
        [filter, data, kioskData],
    );

    const { start, end } = useMemo(
        () =>
            getItemsRangeFromCurrentPage(
                pagination.currentPage,
                filteredData?.length || PAGE_SIZES[0],
            ),
        [filteredData?.length, pagination.currentPage],
    );

    const sortedDataByDisplayImages = useMemo(() => {
        if (!filteredData) {
            return [];
        }

        const hasImageUrl = [];
        const noImageUrl = [];

        for (const obj of filteredData) {
            const displayMeta = obj.data?.display?.data;

            if (displayMeta?.image_url) {
                hasImageUrl.push(obj);
            } else {
                noImageUrl.push(obj);
            }
        }

        return [...hasImageUrl, ...noImageUrl];
    }, [filteredData]);

    const showPagination = getShowPagination(
        filter,
        filteredData?.length || 0,
        pagination.currentPage,
        isFetching,
    );

    const hasAssets = sortedDataByDisplayImages.length > 0;
    const noAssets = !hasAssets && !isPending;

    if (isError) {
        return (
            <div className="pt-2 font-sans font-semibold text-issue-dark">Failed to load NFTs</div>
        );
    }

    return (
        <div className={clsx(!noAssets && 'h-coinsAndAssetsContainer md:h-full')}>
            <div className={clsx('flex h-full overflow-hidden', !showPagination && 'pb-2')}>
                <div className="relative flex h-full w-full flex-col gap-4">
                    <div className="flex w-full flex-col items-start sm:min-h-[72px] sm:flex-row sm:items-center sm:justify-between">
                        <Title size={TitleSize.Medium} title="Assets" />
                        {hasAssets && (
                            <div className="flex flex-row-reverse justify-between sm:flex-row sm:pr-lg">
                                <div className="flex items-center gap-sm">
                                    {VIEW_MODES.map((mode) => {
                                        const selected = mode.value === viewMode;
                                        return (
                                            <div
                                                key={mode.value}
                                                className={clsx(
                                                    'flex h-6 w-6 items-center justify-center',
                                                    selected ? 'text-white' : 'text-steel',
                                                )}
                                            >
                                                <Button
                                                    icon={mode.icon}
                                                    size={ButtonSize.Small}
                                                    type={
                                                        selected
                                                            ? ButtonType.Secondary
                                                            : ButtonType.Ghost
                                                    }
                                                    onClick={() => {
                                                        setViewMode(mode.value);
                                                    }}
                                                />
                                            </div>
                                        );
                                    })}
                                </div>
                                <div className="pl-md pr-md">
                                    <Divider type={DividerType.Vertical} />
                                </div>

                                <SegmentedButton
                                    type={SegmentedButtonType.Outlined}
                                    shape={ButtonSegmentType.Rounded}
                                >
                                    {FILTER_OPTIONS.map((f) => (
                                        <ButtonSegment
                                            key={f.value}
                                            type={ButtonSegmentType.Rounded}
                                            selected={f.value === filter}
                                            label={f.label}
                                            disabled={
                                                (f.value === FilterValue.Kiosks &&
                                                    !kioskData?.list?.length) ||
                                                isPending
                                            }
                                            onClick={() => setFilter(f.value)}
                                        />
                                    ))}
                                </SegmentedButton>
                            </div>
                        )}
                    </div>
                    <div className="flex-2 flex w-full flex-col overflow-hidden p-md">
                        {noAssets && (
                            <div className="flex h-20 items-center justify-center md:h-coinsAndAssetsContainer">
                                <div className="text-body-lg">No Assets owned</div>
                            </div>
                        )}

                        {hasAssets && viewMode === ObjectViewMode.List && (
                            <ListView loading={isPending} data={sortedDataByDisplayImages} />
                        )}
                        {hasAssets && viewMode === ObjectViewMode.SmallThumbnail && (
                            <SmallThumbnailsView
                                loading={isPending}
                                data={sortedDataByDisplayImages}
                                limit={limit}
                            />
                        )}
                        {hasAssets && viewMode === ObjectViewMode.Thumbnail && (
                            <ThumbnailsView
                                loading={isPending}
                                data={sortedDataByDisplayImages}
                                limit={limit}
                            />
                        )}
                    </div>
                    {showPagination && hasAssets && (
                        <div className="flex flex-1 items-end p-md pt-none">
                            <div className="flex w-full flex-row flex-wrap items-center justify-between gap-2">
                                <Pagination {...pagination} />
                                <div className="ml-auto flex items-center">
                                    {!isPending && (
                                        <Text variant="body/medium" color="steel">
                                            Showing {start} - {end}
                                        </Text>
                                    )}
                                </div>
                                <div className="hidden sm:block">
                                    <select
                                        className="form-select rounded-md border border-gray-45 px-3 py-2 pr-8 text-bodySmall font-medium leading-[1.2] text-steel-dark shadow-button"
                                        value={limit}
                                        onChange={(e) => {
                                            setLimit(Number(e.target.value));
                                            pagination.onFirst();
                                        }}
                                    >
                                        {PAGE_SIZES.map((size) => (
                                            <option key={size} value={size}>
                                                {size} Per Page
                                            </option>
                                        ))}
                                    </select>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
