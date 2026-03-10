// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close } from '@iota/apps-ui-icons';
import { Button, ButtonType, Chip, ChipType, LoadingIndicator } from '@iota/apps-ui-kit';
import { useCurrentWallet } from '@iota/dapp-kit';
import { validateIotaName } from '@iota/iota-names-sdk';
import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { AuctionBidDialog } from '@/auctions/components/dialogs/AuctionBidDialog';
import { useGetAuctionMetadata } from '@/auctions/hooks/useGetAuctionMetadata';
import {
    NameRecordData,
    useBlockedList,
    useNameRecord,
    usePriceList,
    useReservedList,
} from '@/hooks';
import { useDebounce } from '@/hooks/useDebounce';
import { useNamesPurchaseMode } from '@/hooks/useNamesPurchaseMode';
import { MY_NAMES_ROUTE } from '@/lib/constants';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';
import { denormalizeName } from '@/lib/utils/format/formatNames';
import { useAvailabilityCheckDialog } from '@/stores/useAvailabilityCheckDialog';

import { ConnectButton } from '../buttons/ConnectButton';
import { PurchaseNameDialog } from '../dialogs/PurchaseNameDialog';
import { NameAvailabilityStatus, NamePurchaseCard } from '../name-purchase-card';
import { SearchStylized } from '../search-component/SearchStylized';

const NAME_REQUEST_FORM_URL =
    'https://docs.google.com/forms/d/e/1FAIpQLSc4LDyCu7QbKDrE1CPfINLO9QSrOsPcZW8sPP-4Zt73N-3fUg/viewform';

interface AvailabilityCheckProps {
    autoFocusInput?: boolean;
}
interface RecentSearch {
    searchedName: string;
    isNotAvailable: boolean;
}

const RECENT_SEARCHES_STORAGE_KEY = 'iota-names-recent-searches';
const DEBOUNCE_DELAY = 750;

export function AvailabilityCheck({ autoFocusInput }: AvailabilityCheckProps) {
    const pathname = usePathname();
    const router = useRouter();
    const { close } = useAvailabilityCheckDialog();
    const { data: blockedList = [], isLoading: isLoadingBlockedList } = useBlockedList();
    const { data: reservedList = [], isLoading: isLoadingReservedList } = useReservedList();
    const [searchValue, setSearchValue] = useState<string>('');
    const debouncedSearchValue = useDebounce(searchValue, DEBOUNCE_DELAY);
    const [recentSearches, setRecentSearches] = useState<RecentSearch[]>(() => {
        const storedRecentSearches = localStorage.getItem(RECENT_SEARCHES_STORAGE_KEY);
        return storedRecentSearches ? (JSON.parse(storedRecentSearches) as RecentSearch[]) : [];
    });

    const name = debouncedSearchValue ? `${debouncedSearchValue}.iota` : '';

    const {
        data: auctionMetadata,
        error: auctionError,
        isLoading: isLoadingAuctionMetadata,
    } = useGetAuctionMetadata(name);
    const {
        data: nameRecordData,
        error: nameError,
        isLoading: isLoadingNameRecord,
    } = useNameRecord(name);
    const { data: priceList, error: priceError, isLoading: isLoadingPriceList } = usePriceList();

    const validationError = useMemo(
        () =>
            searchValue
                ? validateIotaName(
                      `${searchValue}.iota`,
                      priceList?.minLength,
                      priceList?.maxLength,
                      false,
                  )
                : null,
        [searchValue, priceList],
    );

    const errorMessageRaw = auctionError?.message || nameError?.message || priceError?.message;
    const errorMessage =
        validationError || (errorMessageRaw ? getUserFriendlyErrorMessage(errorMessageRaw) : '');

    const isLoading =
        !validationError &&
        debouncedSearchValue &&
        (isLoadingAuctionMetadata ||
            isLoadingNameRecord ||
            isLoadingPriceList ||
            isLoadingBlockedList ||
            isLoadingReservedList);

    const isAuctionInProgress = auctionMetadata?.isActive;
    const isUnavailable = nameRecordData?.type === 'unavailable';
    const isNameTaken = isUnavailable && !isAuctionInProgress;

    useEffect(() => {
        if (!name) {
            return;
        }
        ampli.performedSearch({ query: name });
    }, [name]);

    useEffect(() => {
        if (nameRecordData && searchValue && name === `${searchValue}.iota`) {
            updateRecentSearch(searchValue, isNameTaken);
        }
    }, [nameRecordData, isNameTaken, name, searchValue]);

    function persistRecentSearches(recentSearches: RecentSearch[]) {
        localStorage.setItem(RECENT_SEARCHES_STORAGE_KEY, JSON.stringify(recentSearches));
    }

    function updateRecentSearch(searchedName: string, isNotAvailable: boolean) {
        const MAX_RECENT = 4;

        setRecentSearches((previousSearches) => {
            const updatedRecentSearches = [
                { searchedName, isNotAvailable },
                ...previousSearches.filter((search) => search.searchedName !== searchedName),
            ].slice(0, MAX_RECENT);
            persistRecentSearches(updatedRecentSearches);
            return updatedRecentSearches;
        });
    }

    function removeRecentSearch(searchedNameToRemove: string) {
        setRecentSearches((previousSearches) => {
            const updatedRecentSearches = previousSearches.filter(
                (search) => search.searchedName !== searchedNameToRemove,
            );
            persistRecentSearches(updatedRecentSearches);
            return updatedRecentSearches;
        });
    }

    function handleRecentClick(value: string) {
        setSearchValue(value);
        updateRecentSearch(value, isNameTaken);
    }

    const handleSearch = useCallback(() => {
        const fullName = `${searchValue}.iota`;
        if (fullName === name && nameRecordData) {
            updateRecentSearch(searchValue, isNameTaken);
        }
    }, [searchValue, validationError, isNameTaken, name, nameRecordData]);

    function handleInputChange(inputValue: string) {
        setSearchValue(denormalizeName(inputValue));
    }

    function isForbiddenName(name: string, forbiddenNames: string[]) {
        return forbiddenNames.some((w) => name === `${w}.iota`);
    }

    async function handleCompletedPurchaseOrBid() {
        if (pathname !== MY_NAMES_ROUTE.path) {
            router.push(MY_NAMES_ROUTE.path);
        }
        close();
    }

    return (
        <div className="flex flex-col items-center w-full space-y-4">
            <div className="flex flex-col gap-xl w-full max-w-[744px]">
                <div className="flex flex-col gap-y-md">
                    <div className="flex gap-x-sm items-baseline justify-center w-full">
                        <SearchStylized
                            placeholder="Check name availability"
                            value={searchValue}
                            onChange={({ target: { value } }) => handleInputChange(value)}
                            errorMessage={errorMessage}
                            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
                            leadingIcon={
                                <p className="text-names-neutral-50 text-title-lg xs:text-headline-sm sm:text-headline-md">
                                    @
                                </p>
                            }
                            autoFocus={autoFocusInput}
                            onClearInput={() => {
                                setSearchValue('');
                            }}
                        />
                    </div>
                    {recentSearches?.length > 0 && (
                        <div className="flex flex-wrap gap-xs">
                            <span className="text-body-lg text-names-neutral-50 items-center flex mr-md">
                                Recent
                            </span>
                            {recentSearches.map(({ searchedName, isNotAvailable }) => (
                                <Chip
                                    key={searchedName}
                                    label={searchedName}
                                    type={isNotAvailable ? ChipType.Error : ChipType.Elevated}
                                    trailingElement={
                                        <div
                                            className="[&_svg]:h-4 [&_svg]:w-4 state-layer relative rounded-full cursor-pointer"
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                removeRecentSearch(searchedName);
                                            }}
                                            role="button"
                                            aria-label="Remove from recent searches"
                                        >
                                            <Close />
                                        </div>
                                    }
                                    onClick={() => handleRecentClick(searchedName)}
                                />
                            ))}
                        </div>
                    )}
                </div>
                <div className="flex flex-col items-center space-y-4 w-full">
                    {isLoading ? (
                        <LoadingIndicator />
                    ) : isNameTaken && name ? (
                        <NamePurchaseCard
                            name={name}
                            status={NameAvailabilityStatus.Unavailable}
                            statusMessage="Name is already taken."
                            testId="unavailable-purchase-card"
                        />
                    ) : isForbiddenName(name, blockedList) ? (
                        <NamePurchaseCard
                            name={name}
                            status={NameAvailabilityStatus.Unavailable}
                            disableStatusHoverEffect
                            statusMessage="Name is blocked and cannot be purchased."
                        />
                    ) : isForbiddenName(name, reservedList) ? (
                        <NamePurchaseCard
                            name={name}
                            disableStatusHoverEffect
                            status={NameAvailabilityStatus.Reserved}
                            statusMessage="Submit a request to claim."
                        >
                            <Link
                                href={NAME_REQUEST_FORM_URL}
                                target="_blank"
                                rel="noopener noreferrer"
                            >
                                <Button type={ButtonType.Primary} text="Claim" />
                            </Link>
                        </NamePurchaseCard>
                    ) : nameRecordData?.type === 'not-priced' ? (
                        <NamePurchaseCard
                            name={name}
                            status={NameAvailabilityStatus.Unavailable}
                            statusMessage="Name is not available."
                        />
                    ) : (
                        nameRecordData && (
                            <>
                                <PurchaseName
                                    name={name}
                                    nameRecordData={nameRecordData}
                                    onCompleted={handleCompletedPurchaseOrBid}
                                />

                                <BidName
                                    name={name}
                                    nameRecordData={nameRecordData}
                                    onCompleted={handleCompletedPurchaseOrBid}
                                />
                            </>
                        )
                    )}
                </div>
            </div>
        </div>
    );
}

interface BidNameProps {
    name: string;
    nameRecordData: NameRecordData;
    onCompleted?: () => void;
}
function BidName({ name, nameRecordData, onCompleted }: BidNameProps) {
    const { data: { isAuctionAuthorized } = {} } = useNamesPurchaseMode();
    const { isConnected } = useCurrentWallet();
    const [isAuctionBidDialogOpen, setAuctionDialogOpen] = useState(false);
    const { data: auctionMetadata } = useGetAuctionMetadata(name);

    const isAvailable = nameRecordData?.type === 'available';
    const isUnavailable = nameRecordData?.type === 'unavailable';
    const isAuctionInProgress = auctionMetadata?.isActive;
    const isAllowedToBid =
        (isAvailable && isAuctionAuthorized) || (isUnavailable && isAuctionInProgress) || false;

    if (!isAllowedToBid && !isAuctionAuthorized) {
        return null;
    }

    function handleBid() {
        setAuctionDialogOpen(false);
        onCompleted?.();
    }

    const purchasePrice = nameRecordData?.type === 'available' ? nameRecordData.price : undefined;
    // If there is no auction yet, then we use the purchase price as minimum
    const bidPrice = auctionMetadata?.minBidNanos || purchasePrice;

    return (
        <>
            <NamePurchaseCard
                name={name}
                status={
                    isAllowedToBid
                        ? NameAvailabilityStatus.Available
                        : NameAvailabilityStatus.Unavailable
                }
                priceNanos={bidPrice}
                statusMessage={isAuctionInProgress ? 'In auction' : ''}
                testId="auction-card"
            >
                {isConnected ? (
                    <Button
                        type={ButtonType.Primary}
                        text="Bid"
                        onClick={() => setAuctionDialogOpen(true)}
                    />
                ) : (
                    <ConnectButton />
                )}
            </NamePurchaseCard>

            {isAuctionBidDialogOpen && (
                <AuctionBidDialog
                    name={name}
                    closeDialog={() => setAuctionDialogOpen(false)}
                    onCompleted={handleBid}
                />
            )}
        </>
    );
}

interface PurchaseNameProps {
    name: string;
    nameRecordData: NameRecordData;
    onCompleted?: () => void;
}
function PurchaseName({ name, nameRecordData, onCompleted }: PurchaseNameProps) {
    const { data: { isPaymentAuthorized } = {} } = useNamesPurchaseMode();
    const { isConnected } = useCurrentWallet();
    const [isPurchaseDialogOpen, setPurchaseDialogOpen] = useState(false);

    if (!isPaymentAuthorized) {
        return null;
    }

    const isAvailable = nameRecordData?.type === 'available';

    function handlePurchase() {
        onCompleted?.();
        setPurchaseDialogOpen(false);
    }

    const purchasePrice = nameRecordData?.type === 'available' ? nameRecordData.price : undefined;

    return (
        <>
            {isAvailable && (
                <NamePurchaseCard
                    name={name}
                    status={NameAvailabilityStatus.Available}
                    priceNanos={purchasePrice}
                    testId="purchase-name-card"
                >
                    {isConnected ? (
                        <Button
                            type={ButtonType.Secondary}
                            text="Buy"
                            onClick={() => setPurchaseDialogOpen(true)}
                        />
                    ) : (
                        <ConnectButton />
                    )}
                </NamePurchaseCard>
            )}
            {isPurchaseDialogOpen && (
                <PurchaseNameDialog
                    name={name}
                    open={isPurchaseDialogOpen}
                    setOpen={setPurchaseDialogOpen}
                    onCompleted={handlePurchase}
                />
            )}
        </>
    );
}
