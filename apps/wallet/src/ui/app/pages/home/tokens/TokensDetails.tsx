// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useActiveAccount, useActiveAddress, useAppSelector, useShouldOpenInNewTab } from '_hooks';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { useFeature, useAppsBackendClient } from '@iota/apps-backend-client';
import {
    Feature,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useGetDelegatedStake,
    TIMELOCK_IOTA_TYPE,
    useGetOwnedObjects,
    TIMELOCK_STAKED_TYPE,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetStardustSharedBasicObjects,
    useGetStardustSharedNftObjects,
    useGetAllBalances,
    useGetDefaultIotaName,
    toast,
    haveSupplyIncreaseLabel,
} from '@iota/core';
import {
    Button,
    ButtonSize,
    ButtonType,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
} from '@iota/apps-ui-kit';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { isPasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';
import { formatAccountName } from '../../../helpers';
import { Network } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import {
    ArrowBottomLeft,
    Info,
    IotaLogoMark,
    Keystone,
    Ledger,
    Migration,
    Passkey,
    Send,
    Vesting,
} from '@iota/apps-ui-icons';
import { Interstitial, type InterstitialConfig } from '../interstitial';
import Browser from 'webextension-polyfill';
import { coerce, gte } from 'semver';
import { CoinBalance } from './coin-balance';
import { TokenStakingOverview } from './TokenStakingOverview';
import { Link, useNavigate } from 'react-router-dom';
import { MyTokens } from './MyTokens';
import { ReceiveTokensDialog } from './ReceiveTokensDialog';
import { OverviewHint } from './OverviewHint';
import { SupplyIncreaseVestingStakingDialog } from './SupplyIncreaseVestingStakingDialog';
import { MigrationDialog } from './MigrationDialog';
import { openInNewTab } from '_src/shared/utils';

export function TokenDetails() {
    const navigate = useNavigate();
    const [dialogReceiveOpen, setDialogReceiveOpen] = useState(false);
    const [dialogVestingOpen, setDialogVestingOpen] = useState(false);
    const [dialogMigrationOpen, setDialogMigrationOpen] = useState(false);
    const [interstitialDismissed, setInterstitialDismissed] = useState<boolean>(false);
    const activeCoinType = IOTA_TYPE_ARG;
    const activeAccount = useActiveAccount();
    const activeAccountAddress = activeAccount?.address;
    const { data: iotaName } = useGetDefaultIotaName(activeAccountAddress);
    const accountName = formatAccountName(activeAccount?.nickname, iotaName, activeAccountAddress);
    const network = useAppSelector((state) => state.app.network);
    const shouldOpenNewTab = useShouldOpenInNewTab();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const isFullScreen = extensionViewType === ExtensionViewType.FullScreen;
    const isMainnet = network === Network.Mainnet;
    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;
    const migrationEnabled = useFeature<boolean>(Feature.StardustMigration).value;

    const OBJECT_PER_REQ = 1;

    const client = useAppsBackendClient();
    const { data } = useQuery({
        queryKey: ['apps-backend', 'monitor-network'],
        queryFn: () => client.getMonitorNetwork('WALLET'),
        // Keep cached for 2 minutes:
        staleTime: 2 * 60 * 1000,
        retry: false,
        enabled: isMainnet,
    });
    const address = useActiveAddress();
    const {
        data: coinBalances,
        isPending,
        isLoading,
        isFetched,
        isError,
    } = useGetAllBalances(address);
    const coinBalance = coinBalances?.find((balance) => balance.coinType === activeCoinType);

    const { data: delegatedStake } = useGetDelegatedStake({
        address: activeAccountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const { data: supplyIncreaseVestingObjects } = useGetOwnedObjects(
        activeAccountAddress || '',
        {
            StructType: TIMELOCK_IOTA_TYPE,
        },
        OBJECT_PER_REQ,
    );
    const { data: supplyIncreaseVestingObjectsStaked } = useGetOwnedObjects(
        activeAccountAddress || '',
        {
            StructType: TIMELOCK_STAKED_TYPE,
        },
        OBJECT_PER_REQ,
    );

    const { data: basicOutputObjects } = useGetOwnedObjects(
        activeAccountAddress || '',
        { StructType: STARDUST_BASIC_OUTPUT_TYPE },
        OBJECT_PER_REQ,
    );

    const { data: nftOutputObjects } = useGetOwnedObjects(
        activeAccountAddress || '',
        { StructType: STARDUST_NFT_OUTPUT_TYPE },
        OBJECT_PER_REQ,
    );

    const { data: stardustSharedBasicObjects } = useGetStardustSharedBasicObjects(
        activeAccountAddress || '',
        OBJECT_PER_REQ,
    );
    const { data: stardustSharedNftObjects } = useGetStardustSharedNftObjects(
        activeAccountAddress || '',
        OBJECT_PER_REQ,
    );

    let hasSupplyIncreaseVestingObjects = false;
    let needsMigration = false;

    if (supplyIncreaseVestingEnabled) {
        hasSupplyIncreaseVestingObjects =
            haveSupplyIncreaseLabel(supplyIncreaseVestingObjects?.pages || []) ||
            haveSupplyIncreaseLabel(supplyIncreaseVestingObjectsStaked?.pages || []);
    }

    if (migrationEnabled) {
        needsMigration =
            !!basicOutputObjects?.pages?.[0]?.data?.length ||
            !!nftOutputObjects?.pages?.[0]?.data?.length ||
            !!stardustSharedBasicObjects?.length ||
            !!stardustSharedNftObjects?.length;
    }

    const walletInterstitialConfig = useFeature<InterstitialConfig>(
        Feature.WalletInterstitialConfig,
    ).value;

    const walletVersion = coerce(Browser.runtime.getManifest().version);
    const minVersion = coerce(walletInterstitialConfig?.minVersion);
    const isMinVersionCompatible =
        !minVersion || (!!walletVersion && gte(walletVersion, minVersion));

    const tokenBalance = BigInt(coinBalance?.totalBalance ?? 0);

    // Avoid perpetual loading state when fetching and retry keeps failing add isFetched check
    const isFirstTimeLoading = isPending && !isFetched;

    const onSendClick = () => {
        if (activeAccount) {
            const destination = coinBalance?.coinType
                ? `/send?${new URLSearchParams({ type: coinBalance?.coinType }).toString()}`
                : '/send';

            if (shouldOpenNewTab) {
                openInNewTab(destination);
            } else {
                navigate(destination);
            }
        }
    };

    useEffect(() => {
        const dismissed =
            walletInterstitialConfig?.dismissKey &&
            localStorage.getItem(walletInterstitialConfig.dismissKey);
        setInterstitialDismissed(dismissed === 'true');
    }, [walletInterstitialConfig?.dismissKey]);

    useEffect(() => {
        if (isError) {
            toast.error('Error updating balance');
        }
    }, [isError]);

    if (
        navigator.userAgent !== 'Playwright' &&
        walletInterstitialConfig?.enabled &&
        isMinVersionCompatible &&
        !interstitialDismissed
    ) {
        return (
            <Interstitial
                {...walletInterstitialConfig}
                onClose={() => {
                    setInterstitialDismissed(true);
                }}
            />
        );
    }
    const accountHasIota = coinBalances?.some(({ coinType }) => coinType === IOTA_TYPE_ARG);

    if (!activeAccountAddress) {
        return null;
    }

    return (
        <>
            {isMainnet && data?.degraded && (
                <InfoBox
                    icon={<Info className="h-3 w-3" />}
                    title="App Performance"
                    supportingText="We apologize for the slowdown. Our team is working on a fix and appreciates your patience."
                    type={InfoBoxType.Default}
                    style={InfoBoxStyle.Elevated}
                />
            )}
            <Loading loading={isFirstTimeLoading}>
                {isFullScreen ? (
                    <div className="flex h-full flex-col gap-md" data-testid="coin-page">
                        <div className="flex flex-col gap-md overflow-y-auto scroll-smooth [scrollbar-gutter:stable]">
                            <div className="flex flex-col gap-xs">
                                <AccountProfileLink
                                    account={activeAccount}
                                    accountName={accountName}
                                />
                                <div className="flex w-full items-center justify-between gap-lg px-sm py-md">
                                    <CoinBalance amount={tokenBalance} type={activeCoinType} />
                                    <div className="flex gap-xs [&_svg]:h-5 [&_svg]:w-5">
                                        <Button
                                            onClick={() => setDialogReceiveOpen(true)}
                                            type={ButtonType.Secondary}
                                            icon={<ArrowBottomLeft />}
                                            size={ButtonSize.Small}
                                            testId="receive-coin-button"
                                        />
                                        <Button
                                            onClick={onSendClick}
                                            icon={<Send />}
                                            size={ButtonSize.Small}
                                            disabled={!coinBalances?.length}
                                            testId="send-coin-button"
                                        />
                                    </div>
                                </div>
                            </div>
                            <div className="flex w-full flex-col items-center gap-xs">
                                {accountHasIota || delegatedStake?.length ? (
                                    <TokenStakingOverview accountAddress={activeAccountAddress} />
                                ) : null}
                                {hasSupplyIncreaseVestingObjects || needsMigration ? (
                                    <div className="flex w-full flex-row gap-x-xs">
                                        {needsMigration ? (
                                            <OverviewHint
                                                onClick={() => setDialogMigrationOpen(true)}
                                                title="Migration"
                                                icon={Migration}
                                                subtitle="Action required"
                                            />
                                        ) : null}
                                        {hasSupplyIncreaseVestingObjects ? (
                                            <OverviewHint
                                                onClick={() => setDialogVestingOpen(true)}
                                                title="Vesting"
                                                icon={Vesting}
                                                subtitle="Action required"
                                            />
                                        ) : null}
                                    </div>
                                ) : null}
                                {!accountHasIota ? (
                                    <div className="flex flex-col gap-md">
                                        <div className="flex flex-col flex-nowrap items-center justify-center px-sm text-center">
                                            <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                                {isMainnet
                                                    ? 'Start by buying IOTA'
                                                    : "Need to send transactions on the IOTA network? You'll need IOTA in your wallet"}
                                            </span>
                                        </div>
                                        {!isMainnet && <FaucetRequestButton />}
                                    </div>
                                ) : null}
                            </div>
                        </div>
                        <div className="flex flex-col overflow-y-auto scroll-smooth [scrollbar-gutter:stable]">
                            {coinBalances?.length ? (
                                <MyTokens
                                    coinBalances={coinBalances ?? []}
                                    isLoading={isLoading}
                                    isFetched={isFetched}
                                />
                            ) : null}
                        </div>
                    </div>
                ) : (
                    <div
                        className="flex h-full flex-1 flex-grow flex-col gap-md"
                        data-testid="coin-page"
                    >
                        <div className="flex flex-col gap-xs">
                            <AccountProfileLink account={activeAccount} accountName={accountName} />
                            <div className="flex w-full items-center justify-between gap-lg px-sm py-md">
                                <CoinBalance amount={tokenBalance} type={activeCoinType} />
                                <div className="flex gap-xs [&_svg]:h-5 [&_svg]:w-5">
                                    <Button
                                        onClick={() => setDialogReceiveOpen(true)}
                                        type={ButtonType.Secondary}
                                        icon={<ArrowBottomLeft />}
                                        size={ButtonSize.Small}
                                        testId="receive-coin-button"
                                    />
                                    <Button
                                        onClick={onSendClick}
                                        icon={<Send />}
                                        size={ButtonSize.Small}
                                        disabled={!coinBalances?.length}
                                        testId="send-coin-button"
                                    />
                                </div>
                            </div>
                        </div>
                        <div className="flex min-h-0 w-full flex-grow flex-col gap-md overflow-y-auto scroll-smooth">
                            <div
                                className={`flex w-full flex-col items-center gap-xs rounded-2xl ${!accountHasIota ? 'flex-grow justify-between' : ''}`}
                            >
                                <div className="flex w-full flex-col items-center gap-xs">
                                    {accountHasIota || delegatedStake?.length ? (
                                        <TokenStakingOverview
                                            accountAddress={activeAccountAddress}
                                        />
                                    ) : null}
                                    {hasSupplyIncreaseVestingObjects || needsMigration ? (
                                        <div className="flex w-full flex-row gap-x-xs">
                                            {needsMigration ? (
                                                <OverviewHint
                                                    onClick={() => setDialogMigrationOpen(true)}
                                                    title="Migration"
                                                    icon={Migration}
                                                />
                                            ) : null}
                                            {hasSupplyIncreaseVestingObjects ? (
                                                <OverviewHint
                                                    onClick={() => setDialogVestingOpen(true)}
                                                    title="Vesting"
                                                    icon={Vesting}
                                                />
                                            ) : null}
                                        </div>
                                    ) : null}
                                </div>
                                {!accountHasIota ? (
                                    <div className="flex flex-col gap-md">
                                        <div className="flex flex-col flex-nowrap items-center justify-center px-sm text-center">
                                            <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                                {isMainnet
                                                    ? 'Start by buying IOTA'
                                                    : "Need to send transactions on the IOTA network? You'll need IOTA in your wallet"}
                                            </span>
                                        </div>
                                        {!isMainnet && <FaucetRequestButton />}
                                    </div>
                                ) : null}
                            </div>
                            {coinBalances?.length ? (
                                <MyTokens
                                    coinBalances={coinBalances ?? []}
                                    isLoading={isLoading}
                                    isFetched={isFetched}
                                />
                            ) : null}
                        </div>
                    </div>
                )}
                <ReceiveTokensDialog
                    address={activeAccountAddress}
                    open={dialogReceiveOpen}
                    setOpen={(isOpen) => setDialogReceiveOpen(isOpen)}
                />
                <SupplyIncreaseVestingStakingDialog
                    open={dialogVestingOpen}
                    setOpen={(isOpen) => setDialogVestingOpen(isOpen)}
                />
                <MigrationDialog
                    open={dialogMigrationOpen}
                    setOpen={(isOpen) => setDialogMigrationOpen(isOpen)}
                />
            </Loading>
        </>
    );
}

function AccountProfileLink({
    account,
    accountName,
}: {
    account: SerializedUIAccount | null;
    accountName: string;
}) {
    const isLedgerAccount = account && isLedgerAccountSerializedUI(account);
    const isKeystoneAccount = account && isKeystoneAccountSerializedUI(account);
    const isPasskeyAccount = account && isPasskeyAccountSerializedUI(account);

    return (
        <Link
            to="/accounts/manage"
            data-testid="accounts-manage"
            data-amp-mask
            className="flex w-fit items-center gap-sm rounded-full px-sm py-xs no-underline transition-colors hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8"
        >
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-iota-primary-30 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white">
                {isLedgerAccount ? (
                    <Ledger />
                ) : isKeystoneAccount ? (
                    <Keystone />
                ) : isPasskeyAccount ? (
                    <Passkey />
                ) : (
                    <IotaLogoMark />
                )}
            </div>
            <span className="navbar-item-label-color truncate text-label-lg">{accountName}</span>
        </Link>
    );
}
