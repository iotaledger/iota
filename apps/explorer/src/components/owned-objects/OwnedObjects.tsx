// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useGetKioskContents,
    useGetOwnedObjects,
    useLocalStorage,
    useCursorPagination,
    useIotaNamesClient,
    hasDisplayData,
} from '@iota/core';
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
import { getNameRegistrationType, getSubnameRegistrationType } from '@iota/iota-names-sdk';
import type { IotaObjectResponse } from '@iota/iota-sdk/src/client';

const SHOW_PAGINATION_MAX_ITEMS = 9;
const OWNED_OBJECTS_LOCAL_STORAGE_VIEW_MODE = 'owned-objects/viewMode';
const OWNED_OBJECTS_LOCAL_STORAGE_FILTER = 'owned-objects/filter';

interface ItemsRangeFromCurrentPage {
    start: number;
    end: number;
}

enum FilterValue {
    Unknown = 'unknown',
    Kiosks = 'kiosks',
    Names = 'names',
    Nfts = 'nfts',
}

enum OwnedObjectsContainerHeight {
    Small = 'h-[400px]',
    Default = 'h-[400px] md:h-[570px]',
}

const FILTER_OPTIONS = [
    { label: 'NFTS', value: FilterValue.Nfts },
    { label: 'KIOSKS', value: FilterValue.Kiosks },
    { label: 'NAMES', value: FilterValue.Names },
    { label: 'UNKNOWN', value: FilterValue.Unknown },
];

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

    const [ownedObjectsContainerHeight, setOwnedObjectsContainerHeight] =
        useState<OwnedObjectsContainerHeight>(OwnedObjectsContainerHeight.Small);

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

    const { iotaNamesClient } = useIotaNamesClient();

    const packageId = iotaNamesClient?.getPackage('packageId', 'v1');

    const nameTypes = packageId
        ? [getNameRegistrationType(packageId), getSubnameRegistrationType(packageId)]
        : [];

    const categorizedObjects = useMemo(() => {
        const kiosks = kioskData?.list ?? [];
        const names: IotaObjectResponse[] = [];
        const nfts: IotaObjectResponse[] = [];
        const unknown: IotaObjectResponse[] = [];

        for (const obj of data?.data ?? []) {
            const isIotaName = !!obj.data?.type && nameTypes.includes(obj.data.type);

            if (isIotaName) {
                names.push(obj);
                continue;
            }

            if (hasDisplayData(obj)) {
                nfts.push(obj);
                continue;
            }

            unknown.push(obj);
        }

        return { kiosks, names, nfts, unknown };
    }, [data?.data, kioskData?.list, nameTypes]);

    const availableFilters = useMemo(() => {
        const options: FilterValue[] = [];

        if (categorizedObjects.nfts.length) {
            options.push(FilterValue.Nfts);
        }
        if (categorizedObjects.kiosks.length) {
            options.push(FilterValue.Kiosks);
        }
        if (categorizedObjects.names.length) {
            options.push(FilterValue.Names);
        }
        if (categorizedObjects.unknown.length) {
            options.push(FilterValue.Unknown);
        }

        return options;
    }, [
        categorizedObjects.kiosks.length,
        categorizedObjects.names.length,
        categorizedObjects.unknown.length,
        categorizedObjects.nfts.length,
    ]);

    const isPending = filter === FilterValue.Kiosks ? kioskDataFetching : isFetching;

    useEffect(() => {
        if (!isPending && availableFilters.length) {
            if (!filter || !availableFilters.includes(filter as FilterValue)) {
                setFilter(availableFilters[0]);
                return;
            }
        }
    }, [filter, availableFilters, isPending, setFilter]);

    const filteredData = useMemo(() => {
        if (!data?.data && filter !== FilterValue.Kiosks) return [];

        switch (filter) {
            case FilterValue.Kiosks:
                return categorizedObjects.kiosks;
            case FilterValue.Names:
                return categorizedObjects.names;
            case FilterValue.Nfts:
                return categorizedObjects.nfts;
            case FilterValue.Unknown:
                return categorizedObjects.unknown;
            default:
                return [];
        }
    }, [filter, data?.data, categorizedObjects]);

    const { start, end } = useMemo(
        () => getItemsRangeFromCurrentPage(pagination.currentPage, limit, filteredData?.length),
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
        isPending,
    );

    const hasVisualAssets = sortedDataByDisplayImages.length > 0;

    const noVisualAssets = !hasVisualAssets && !isPending;

    useEffect(() => {
        const ownedObjectsCount = sortedDataByDisplayImages.length;
        let nextHeight = OwnedObjectsContainerHeight.Small;

        Object.keys(MIN_OBJECT_COUNT_TO_HEIGHT_MAP).forEach((minObjectCount) => {
            if (ownedObjectsCount >= Number(minObjectCount)) {
                nextHeight = MIN_OBJECT_COUNT_TO_HEIGHT_MAP[Number(minObjectCount)];
            }
        });

        if (nextHeight !== ownedObjectsContainerHeight) {
            setOwnedObjectsContainerHeight(nextHeight);
        }
    }, [sortedDataByDisplayImages.length, ownedObjectsContainerHeight]);

    if (isError) {
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
                    <div className="flex w-full flex-col flex-wrap items-start justify-between sm:min-h-[72px] sm:flex-row sm:items-center">
                        <Title size={TitleSize.Medium} title="Assets" />
                        {hasVisualAssets && availableFilters.length > 0 && (
                            <div className="flex flex-col gap-sm px-md--rs sm:flex-row sm:gap-0">
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
                                <div className="hidden pl-md pr-md sm:flex">
                                    <Divider type={DividerType.Vertical} />
                                </div>

                                <SegmentedButton
                                    type={SegmentedButtonType.Outlined}
                                    shape={ButtonSegmentType.Rounded}
                                >
                                    {availableFilters.map((value) => {
                                        const option = FILTER_OPTIONS.find(
                                            (opt) => opt.value === value,
                                        );

                                        return (
                                            <ButtonSegment
                                                key={value}
                                                type={ButtonSegmentType.Rounded}
                                                selected={value === filter}
                                                label={option?.label ?? value.toUpperCase()}
                                                disabled={isPending}
                                                onClick={() => setFilter(value)}
                                            />
                                        );
                                    })}
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
                            {hasVisualAssets && viewMode === ObjectViewMode.List && (
                                <ListView loading={isPending} data={sortedDataByDisplayImages} />
                            )}
                            {hasVisualAssets && viewMode === ObjectViewMode.SmallThumbnail && (
                                <SmallThumbnailsView
                                    loading={isPending}
                                    data={sortedDataByDisplayImages}
                                    limit={limit}
                                />
                            )}
                            {hasVisualAssets && viewMode === ObjectViewMode.Thumbnail && (
                                <ThumbnailsView
                                    loading={isPending}
                                    data={sortedDataByDisplayImages}
                                    limit={limit}
                                />
                            )}
                        </div>
                    )}

                    {showPagination && hasVisualAssets && (
                        <div className="flex flex-col items-center justify-between gap-sm px-sm--rs py-sm--rs md:flex-row">
                            <Pagination {...pagination} />
                            <div className="flex items-center gap-3">
                                {!isPending && (
                                    <span className="shrink-0 text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                        Showing {start} - {end}
                                    </span>
                                )}
                                <Select
                                    dropdownPosition={DropdownPosition.Top}
                                    value={limit.toString()}
                                    options={PAGE_SIZES_RANGE_10_50.map((size) => ({
                                        label: `${size} / page`,
                                        id: size.toString(),
                                    }))}
                                    onValueChange={(value) => {
                                        setLimit(Number(value));
                                        pagination.onFirst();
                                    }}
                                    size={SelectSize.Small}
                                />
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
