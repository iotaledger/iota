// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLinkType, Loading, UnlockAccountButton } from '_components';
import {
    useActiveAccount,
    useAppSelector,
    useCoinsReFetchingConfig,
    useExplorerLink,
} from '_hooks';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { useFeature } from '@growthbook/growthbook-react';
import { toast } from 'react-hot-toast';
import {
    Feature,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    filterAndSortTokenBalances,
    useAppsBackend,
    useBalance,
    useGetDelegatedStake,
} from '@iota/core';
import {
    Button,
    ButtonSize,
    ButtonType,
    Address,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Network } from '@iota/iota-sdk/client';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { ArrowBottomLeft, Info, Send } from '@iota/apps-ui-icons';
import { Interstitial, type InterstitialConfig } from '../interstitial';
import { CoinBalance } from './coin-balance';
import { TokenStakingOverview } from './TokenStakingOverview';
import { useNavigate } from 'react-router-dom';
import { MyTokens } from './MyTokens';
import { ReceiveTokensDialog } from './ReceiveTokensDialog';

interface TokenDetailsProps {
    coinType?: string;
}

export function TokenDetails({ coinType }: TokenDetailsProps) {
    const [dialogReceiveOpen, setDialogReceiveOpen] = useState(false);
    const navigate = useNavigate();
    const [interstitialDismissed, setInterstitialDismissed] = useState<boolean>(false);
    const activeCoinType = coinType || IOTA_TYPE_ARG;
    const activeAccount = useActiveAccount();
    const activeAccountAddress = activeAccount?.address;
    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();
    const {
        data: coinBalance,
        isError,
        isPending,
        isFetched,
    } = useBalance(activeAccountAddress!, { coinType: activeCoinType });
    const network = useAppSelector((state) => state.app.network);
    const isMainnet = network === Network.Mainnet;
    const { request } = useAppsBackend();
    const { data } = useQuery({
        queryKey: ['apps-backend', 'monitor-network'],
        queryFn: () =>
            request<{ degraded: boolean }>('monitor-network', {
                project: 'WALLET',
            }),
        // Keep cached for 2 minutes:
        staleTime: 2 * 60 * 1000,
        retry: false,
        enabled: isMainnet,
    });
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: activeAccountAddress,
    });

    const {
        data: coinBalances,
        isPending: coinBalancesLoading,
        isFetched: coinBalancesFetched,
    } = useIotaClientQuery(
        'getAllBalances',
        { owner: activeAccountAddress! },
        {
            enabled: !!activeAccountAddress,
            staleTime,
            refetchInterval,
            select: filterAndSortTokenBalances,
        },
    );

    const { data: delegatedStake } = useGetDelegatedStake({
        address: activeAccountAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const walletInterstitialConfig = useFeature<InterstitialConfig>(
        Feature.WalletInterstitialConfig,
    ).value;

    const tokenBalance = BigInt(coinBalance?.totalBalance ?? 0);

    // Avoid perpetual loading state when fetching and retry keeps failing add isFetched check
    const isFirstTimeLoading = isPending && !isFetched;

    const onSendClick = () => {
        if (!activeAccount?.isLocked) {
            const destination = coinBalance?.coinType
                ? `/send?${new URLSearchParams({ type: coinBalance?.coinType }).toString()}`
                : '/send';

            navigate(destination);
        }
    };

    useEffect(() => {
        const dismissed =
            walletInterstitialConfig?.dismissKey &&
            localStorage.getItem(walletInterstitialConfig.dismissKey);
        setInterstitialDismissed(dismissed === 'true');
    }, [walletInterstitialConfig?.dismissKey]);

    if (
        navigator.userAgent !== 'Playwright' &&
        walletInterstitialConfig?.enabled &&
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
    if (isError) {
        toast.error('Error updating balance');
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
                <div
                    className="flex h-full flex-1 flex-grow flex-col items-center gap-md"
                    data-testid="coin-page"
                >
                    <div className="flex w-full items-center justify-between gap-lg px-sm py-lg">
                        <div className="flex flex-col gap-xs">
                            <Address
                                isExternal={!!explorerHref}
                                externalLink={explorerHref!}
                                text={formatAddress(activeAccountAddress)}
                                isCopyable
                                copyText={activeAccountAddress}
                                onCopySuccess={() => toast.success('Address copied')}
                            />
                            <CoinBalance amount={tokenBalance} type={activeCoinType} />
                        </div>
                        <div className="flex gap-xs [&_svg]:h-5 [&_svg]:w-5">
                            <Button
                                onClick={() => setDialogReceiveOpen(true)}
                                type={ButtonType.Secondary}
                                icon={<ArrowBottomLeft />}
                                size={ButtonSize.Small}
                                disabled={activeAccount?.isLocked}
                            />
                            <Button
                                onClick={onSendClick}
                                icon={<Send />}
                                size={ButtonSize.Small}
                                disabled={activeAccount?.isLocked || !coinBalances?.length}
                                testId="send-coin-button"
                            />
                        </div>
                    </div>
                    {activeAccount.isLocked ? (
                        <UnlockAccountButton account={activeAccount} />
                    ) : (
                        <div className="flex w-full flex-col gap-md">
                            <div className="flex w-full flex-col items-center gap-sm rounded-2xl">
                                {!accountHasIota ? (
                                    <div className="flex flex-col gap-md">
                                        <div className="flex flex-col flex-nowrap items-center justify-center px-sm text-center">
                                            <span className="text-body-sm text-neutral-40">
                                                {isMainnet
                                                    ? 'Start by buying IOTA'
                                                    : 'Need to send transactions on the IOTA network? You’ll need IOTA in your wallet'}
                                            </span>
                                        </div>
                                        {!isMainnet && <FaucetRequestButton />}
                                    </div>
                                ) : null}
                                {accountHasIota || delegatedStake?.length ? (
                                    <TokenStakingOverview
                                        disabled={!tokenBalance}
                                        accountAddress={activeAccountAddress}
                                    />
                                ) : null}
                            </div>
                            {coinBalances?.length ? (
                                <MyTokens
                                    coinBalances={coinBalances ?? []}
                                    isLoading={coinBalancesLoading}
                                    isFetched={coinBalancesFetched}
                                />
                            ) : null}
                        </div>
                    )}
                </div>
                <ReceiveTokensDialog
                    address={activeAccountAddress}
                    open={dialogReceiveOpen}
                    setOpen={(isOpen) => setDialogReceiveOpen(isOpen)}
                />
            </Loading>
        </>
    );
}
