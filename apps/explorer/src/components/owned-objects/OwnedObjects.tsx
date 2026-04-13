// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetCategorizedOwnedObjects, OwnedObjectCategory, useLocalStorage } from '@iota/core';
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
    Select,
    DropdownPosition,
    SelectSize,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { ListViewLarge, ListViewMedium, ListViewSmall, Warning } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { useEffect, useMemo, useState } from 'react';
import { ListView, NoObjectsOwnedMessage, SmallThumbnailsView, ThumbnailsView } from '~/components';
import { ObjectViewMode } from '~/lib/enums';
import { Pagination } from '~/components/ui';
import { PAGE_SIZES_RANGE_10_50 } from '~/lib/constants';

const SHOW_PAGINATION_MAX_ITEMS = 9;
const OWNED_OBJECTS_LOCAL_STORAGE_VIEW_MODE = 'owned-objects/viewMode';
const OWNED_OBJECTS_LOCAL_STORAGE_FILTER = 'owned-objects/filter';
const CATEGORY_LABELS: Record<OwnedObjectCategory, string> = {
    [OwnedObjectCategory.Nft]: 'NFT',
    [OwnedObjectCategory.Name]: 'NAME',
    [OwnedObjectCategory.Kiosk]: 'KIOSK',
    [OwnedObjectCategory.Other]: 'OTHER',
};
interface ItemsRangeFromCurrentPage {
    start: number;
    end: number;
}

enum OwnedObjectsContainerHeight {
    Small = 'h-[400px]',
    Default = 'h-[400px] md:h-[570px]',
}

const VIEW_MODES = [
    { icon: <ListViewSmall />, value: ObjectViewMode.List },
    { icon: <ListViewMedium />, value: ObjectViewMode.SmallThumbnail },
    { icon: <ListViewLarge />, value: ObjectViewMode.Thumbnail },
];

function getItemsRangeFromCurrentPage(
    currentPage: number,
    itemsPerPage: number,
    availableItems?: number,
): ItemsRangeFromCurrentPage {
    const start = currentPage * itemsPerPage + 1;
    let end = start + itemsPerPage - 1;

    if (availableItems && availableItems < itemsPerPage) {
        end = start + availableItems - 1;
    }

    return { start, end };
}

function getShowPagination(itemsLength: number, currentPage: number, isFetching: boolean): boolean {
    if (isFetching) {
        return true;
    }

    return currentPage !== 0 || itemsLength > SHOW_PAGINATION_MAX_ITEMS;
}

const MIN_OBJECT_COUNT_TO_HEIGHT_MAP: Record<number, OwnedObjectsContainerHeight> = {
    0: OwnedObjectsContainerHeight.Small,
    20: OwnedObjectsContainerHeight.Default,
};

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

    const ownedObjects = useGetCategorizedOwnedObjects(id, limit);

    const activeCategory = (filter as OwnedObjectCategory) ?? undefined;

    const activeCategoryData = (() => {
        switch (activeCategory) {
            case OwnedObjectCategory.Nft:
                return ownedObjects.nft;
            case OwnedObjectCategory.Name:
                return ownedObjects.name;
            case OwnedObjectCategory.Kiosk:
                return ownedObjects.kiosk;
            case OwnedObjectCategory.Other:
                return ownedObjects.other;
            default:
                return undefined;
        }
    })();

    const { availableCategories, isPending: isLoading } = ownedObjects;

    useEffect(() => {
        if (!isLoading && availableCategories.length) {
            if (!filter || !availableCategories.includes(filter as OwnedObjectCategory)) {
                setFilter(availableCategories[0]);
                return;
            }
        }
    }, [filter, availableCategories, isLoading, setFilter]);

    const isFilterSettled =
        !isLoading && !!filter && availableCategories.includes(filter as OwnedObjectCategory);
    const isPending = isLoading || (!isFilterSettled && availableCategories.length > 0);

    const effectiveViewMode = filter === OwnedObjectCategory.Other ? ObjectViewMode.List : viewMode;

    const availableViewModes =
        filter === OwnedObjectCategory.Other
            ? VIEW_MODES.filter((mode) => mode.value === ObjectViewMode.List)
            : VIEW_MODES;

    const filteredData = activeCategoryData?.data ?? [];

    const { start, end } = useMemo(
        () =>
            getItemsRangeFromCurrentPage(
                activeCategoryData?.pagination.currentPage ?? 0,
                limit,
                filteredData?.length,
            ),
        [filteredData?.length, activeCategoryData?.pagination.currentPage, limit],
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

    const ownedObjectsCount = sortedDataByDisplayImages.length;
    let ownedObjectsContainerHeight = OwnedObjectsContainerHeight.Small;

    for (const minObjectCount of Object.keys(MIN_OBJECT_COUNT_TO_HEIGHT_MAP)) {
        if (ownedObjectsCount >= Number(minObjectCount)) {
            ownedObjectsContainerHeight = MIN_OBJECT_COUNT_TO_HEIGHT_MAP[Number(minObjectCount)];
        }
    }

    const showPagination = getShowPagination(
        filteredData?.length || 0,
        activeCategoryData?.pagination.currentPage ?? 0,
        isPending,
    );

    const hasVisualAssets = isPending || sortedDataByDisplayImages.length > 0;

    const noVisualAssets = !isPending && sortedDataByDisplayImages.length === 0;

    if (ownedObjects.isAnyError) {
        return (
            <div className="p-sm--rs">
                <InfoBox
                    title="Error"
                    supportingText="Failed to load Assets"
                    icon={<Warning />}
                    type={InfoBoxType.Error}
                    style={InfoBoxStyle.Default}
                />
            </div>
        );
    }

    return (
        <div className={clsx(!noVisualAssets ? 'h-coinsAndAssetsContainer' : 'h-full')}>
            <div className={clsx('flex h-full overflow-hidden', !showPagination && 'pb-2')}>
                <div
                    className={clsx('relative flex h-full w-full flex-col', {
                        'gap-4': hasVisualAssets,
                    })}
                >
                    <div className="flex w-full flex-col flex-wrap items-start justify-between gap-xs sm:min-h-[72px] sm:flex-row sm:items-center md:gap-0">
                        <Title size={TitleSize.Medium} title="Assets" />
                        {hasVisualAssets && availableCategories.length > 0 && (
                            <div className="flex flex-col gap-sm px-md--rs sm:flex-row sm:gap-0">
                                <div className="flex items-center gap-sm">
                                    {availableViewModes.map((mode) => {
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
                                                    aria-label={
                                                        mode.value === ObjectViewMode.List
                                                            ? 'List view'
                                                            : mode.value ===
                                                                ObjectViewMode.SmallThumbnail
                                                              ? 'Small thumbnail view'
                                                              : 'Thumbnail view'
                                                    }
                                                />
                                            </div>
                                        );
                                    })}
                                </div>
                                <div className="hidden pl-md pr-md sm:flex">
                                    <Divider type={DividerType.Vertical} />
                                </div>

                                <SegmentedButton
                                    type={SegmentedButtonType.Outlined}
                                    shape={ButtonSegmentType.Rounded}
                                >
                                    {availableCategories.map((value) => (
                                        <ButtonSegment
                                            key={value}
                                            type={ButtonSegmentType.Rounded}
                                            selected={value === filter}
                                            label={CATEGORY_LABELS[value]}
                                            disabled={isPending}
                                            onClick={() => setFilter(value)}
                                        />
                                    ))}
                                </SegmentedButton>
                            </div>
                        )}
                    </div>
                    {noVisualAssets ? (
                        <NoObjectsOwnedMessage objectType="Assets" />
                    ) : (
                        <div
                            className={clsx(
                                'flex-2 flex w-full flex-col overflow-hidden p-md',
                                ownedObjectsContainerHeight,
                            )}
                        >
                            {hasVisualAssets && effectiveViewMode === ObjectViewMode.List && (
                                <ListView loading={isPending} data={sortedDataByDisplayImages} />
                            )}
                            {hasVisualAssets &&
                                effectiveViewMode === ObjectViewMode.SmallThumbnail && (
                                    <SmallThumbnailsView
                                        loading={isPending}
                                        data={sortedDataByDisplayImages}
                                        limit={limit}
                                    />
                                )}
                            {hasVisualAssets && effectiveViewMode === ObjectViewMode.Thumbnail && (
                                <ThumbnailsView
                                    loading={isPending}
                                    data={sortedDataByDisplayImages}
                                    limit={limit}
                                />
                            )}
                        </div>
                    )}

                    <div className="flex flex-col items-center justify-between gap-sm px-sm--rs py-sm--rs md:flex-row">
                        {showPagination && hasVisualAssets && activeCategoryData && (
                            <Pagination {...activeCategoryData.pagination} />
                        )}
                        <div className="flex items-center gap-sm">
                            {!isPending && showPagination && hasVisualAssets && (
                                <span className="shrink-0 text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                    Showing {start} - {end}
                                </span>
                            )}
                            {showPagination && hasVisualAssets && (
                                <Select
                                    dropdownPosition={DropdownPosition.Top}
                                    value={limit.toString()}
                                    options={PAGE_SIZES_RANGE_10_50.map((size) => ({
                                        label: `${size} / page`,
                                        id: size.toString(),
                                    }))}
                                    onValueChange={(value) => {
                                        setLimit(Number(value));
                                        activeCategoryData?.pagination.onFirst();
                                    }}
                                    size={SelectSize.Small}
                                />
                            )}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
