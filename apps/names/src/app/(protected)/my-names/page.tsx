// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Add, Info, Refresh } from '@iota/apps-ui-icons';
import {
    Badge,
    BadgeType,
    Button,
    ButtonSegment,
    ButtonSegmentType,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    SegmentedButton,
} from '@iota/apps-ui-kit';
import { useCurrentAccount } from '@iota/dapp-kit';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import toast from 'react-hot-toast';

import { RenewSubnameDialog } from '@/components/dialogs/RenewSubameDialog';
import { ExtendedNameCard } from '@/components/name-card/ExtendedNameCard';
import { CardSkeletonLoader } from '@/components/skeletons/CardSkeletonLoader';
import { useGetPublicName, useRegistrationNfts } from '@/hooks';
import { RegistrationNft } from '@/lib/interfaces';
import { useAvailabilityCheckDialog } from '@/stores/useAvailabilityCheckDialog';

import { SubnamesDialog } from './components/SubnamesDialog';
import { GroupedNamesFilter } from './filters';

export default function MyNamesPage(): JSX.Element {
    const { open } = useAvailabilityCheckDialog();
    const queryClient = useQueryClient();

    const [selectedNameForRenewal, setSelectedNameForRenewal] = useState<RegistrationNft | null>(
        null,
    );
    const [rightPanelSelectedName, setRightPanelSelectedName] = useState<RegistrationNft | null>(
        null,
    );
    const [selectedFilter, setSelectedFilter] = useState<GroupedNamesFilter>(
        GroupedNamesFilter.All,
    );
    const [isRefreshing, setIsRefreshing] = useState(false);

    const {
        data: names,
        error: isNamesErrored,
        isLoading: isLoadingRegistrations,
    } = useRegistrationNfts('name');
    const {
        data: subnames,
        error: isSubnamesErrored,
        isLoading: isLoadingSubnames,
    } = useRegistrationNfts('subname');

    const address = useCurrentAccount()?.address ?? '';
    const { data: publicName } = useGetPublicName(address);

    const isLoadingCards = isLoadingSubnames || isLoadingRegistrations;

    const filteredNames: RegistrationNft[] = (() => {
        const namesRegistrations = names ?? [];
        const subnamesRegistrations = subnames ?? [];

        switch (selectedFilter) {
            case GroupedNamesFilter.All:
                return [...namesRegistrations, ...subnamesRegistrations];
            case GroupedNamesFilter.Names:
                return namesRegistrations;
            case GroupedNamesFilter.Subnames:
                return subnamesRegistrations;
            default:
                return namesRegistrations;
        }
    })();

    const totalItemsCount = filteredNames.length;

    const noCardToDisplay =
        !isLoadingCards && filteredNames.length === 0 && !isNamesErrored && !isSubnamesErrored;

    function handleFilterSelect(filter: GroupedNamesFilter): void {
        setSelectedFilter(filter);
        closePanel();
    }

    function isPublicName(name: RegistrationNft): boolean {
        return publicName ? publicName === name.name : false;
    }

    function handleNameRenewed(name: RegistrationNft): void {
        closePanel();
        toast.success(
            `${normalizeIotaName(name.name, 'at', {
                truncateLongParts: true,
            })} renewed successfully`,
        );
    }

    function closePanel() {
        setRightPanelSelectedName(null);
    }

    async function handleRefresh(): Promise<void> {
        if (isRefreshing) return;

        setIsRefreshing(true);
        try {
            // Invalidate names and subnames queries to fetch fresh data
            await queryClient.invalidateQueries({
                queryKey: ['iota-name', 'owned-objects'],
            });

            toast.success('Refreshed successfully');
        } catch (error) {
            toast.error('Failed to refresh data');
        } finally {
            setIsRefreshing(false);
        }
    }

    return (
        <>
            <div className="flex flex-row gap-md items-center pt-[80px] md:pt-0 btn-alt-bg">
                <h2 className="text-headline-md text-names-neutral-92 font-bold leading-[120%] -tracking-[0.4px]">
                    My Names
                </h2>

                <div className="flex gap-sm">
                    <Button
                        type={ButtonType.Outlined}
                        text="Name"
                        icon={<Add />}
                        onClick={() =>
                            open({
                                autoFocusInput: true,
                            })
                        }
                    />
                    {selectedFilter === GroupedNamesFilter.All && (
                        <Button
                            type={ButtonType.Outlined}
                            icon={
                                isRefreshing ? (
                                    <LoadingIndicator size="w-5 h-5" />
                                ) : (
                                    <Refresh className="w-5 h-5" />
                                )
                            }
                            onClick={handleRefresh}
                            disabled={isRefreshing}
                            testId="refresh-button"
                            aria-label="Refresh"
                        />
                    )}
                </div>
            </div>

            <div className="flex flex-col sm:flex-row items-start sm:items-center gap-md">
                <SegmentedButton>
                    {Object.entries(GroupedNamesFilter).map(([key, value]) => (
                        <ButtonSegment
                            key={key}
                            type={ButtonSegmentType.Rounded}
                            label={value}
                            selected={selectedFilter === value}
                            onClick={() => handleFilterSelect(value)}
                            disabled={
                                (value === GroupedNamesFilter.Names && !names?.length) ||
                                (value === GroupedNamesFilter.Subnames && !subnames?.length)
                            }
                        />
                    ))}
                </SegmentedButton>
                {!isLoadingCards && (
                    <p className="text-label-md whitespace-nowrap text-names-neutral-70 ml-2 sm:ml-0">
                        {totalItemsCount} Total
                    </p>
                )}
            </div>

            {noCardToDisplay ? (
                <div className="flex">
                    <InfoBox
                        style={InfoBoxStyle.Elevated}
                        type={InfoBoxType.Default}
                        supportingText={`You don't own any ${selectedFilter === GroupedNamesFilter.Subnames ? 'subnames' : 'names'} yet.`}
                        icon={<Info />}
                    />
                </div>
            ) : null}

            {isLoadingCards && (
                <div className="flex w-full justify-start">
                    <CardSkeletonLoader />
                </div>
            )}

            {((!isLoadingCards && filteredNames.length > 0) || rightPanelSelectedName) && (
                <div className="flex flex-row items-start justify-between gap-xl">
                    <div className="gap-lg w-full flex flex-row flex-wrap items-center justify-center sm:justify-start">
                        {filteredNames.map((nft) => (
                            <ExtendedNameCard
                                key={nft.name}
                                nft={nft}
                                onSubnameListClick={() => {
                                    setRightPanelSelectedName(nft);
                                }}
                                isActive={rightPanelSelectedName?.name === nft.name}
                                badge={
                                    isPublicName(nft) ? (
                                        <Badge type={BadgeType.PrimarySolid} label="Public Name" />
                                    ) : null
                                }
                            />
                        ))}
                    </div>

                    {rightPanelSelectedName && (
                        <>
                            <SubnamesDialog
                                selectedName={rightPanelSelectedName}
                                onClose={closePanel}
                                onRenewClick={(name) => setSelectedNameForRenewal(name)}
                            />
                        </>
                    )}
                </div>
            )}
            {selectedNameForRenewal && (
                <RenewSubnameDialog
                    name={selectedNameForRenewal.name}
                    setOpen={() => setSelectedNameForRenewal(null)}
                    onRenew={() => handleNameRenewed(selectedNameForRenewal)}
                />
            )}
        </>
    );
}
