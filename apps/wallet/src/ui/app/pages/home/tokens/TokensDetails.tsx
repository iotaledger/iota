// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIsWalletDefiEnabled } from '_app/hooks/useIsWalletDefiEnabled';
import { LargeButton } from '_app/shared/LargeButton';
import { Text } from '_app/shared/text';
import { ButtonOrLink } from '_app/shared/utils/ButtonOrLink';
import Alert from '_components/alert';
import { CoinIcon } from '_components/coin-icon';
import Loading from '_components/loading';
import { useAppSelector, useCoinsReFetchingConfig } from '_hooks';
import { ampli } from '_src/shared/analytics/ampli';
import { Feature } from '_src/shared/experimentation/features';
import { AccountsList } from '_src/ui/app/components/accounts/AccountsList';
import { UnlockAccountButton } from '_src/ui/app/components/accounts/UnlockAccountButton';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';
import { usePinnedCoinTypes } from '_src/ui/app/hooks/usePinnedCoinTypes';
import FaucetRequestButton from '_src/ui/app/shared/faucet/FaucetRequestButton';
import PageTitle from '_src/ui/app/shared/PageTitle';
import { useFeature } from '@growthbook/growthbook-react';
import {
    filterAndSortTokenBalances,
    useAppsBackend,
    useBalance,
    useBalanceInUSD,
    useCoinMetadata,
    useFormatCoin,
    useGetDelegatedStake,
    useResolveIotaNSName,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    useSortedCoinsByCategories,
} from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Info12 } from '@iota/icons';
import { Network, type CoinBalance as CoinBalanceType } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import { useEffect, useState, type ReactNode } from 'react';
import { Unpined, Pined } from '@iota/ui-icons';
import Interstitial, { type InterstitialConfig } from '../interstitial';
import { CoinBalance } from './coin-balance';
import { PortfolioName } from './PortfolioName';
import { TokenIconLink } from './TokenIconLink';
import { TokenLink } from './TokenLink';
import { TokenList } from './TokenList';

interface TokenDetailsProps {
    coinType?: string;
}

interface PinButtonProps {
    isPinned?: boolean;
    onClick: () => void;
}

function PinButton({ isPinned, onClick }: PinButtonProps) {
    return (
        <button
            type="button"
            className="cursor-pointer border-none bg-transparent [&_svg]:h-4 [&_svg]:w-4"
            aria-label={isPinned ? 'Unpin Coin' : 'Pin Coin'}
            onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onClick();
            }}
        >
            {isPinned ? (
                <Pined className="text-primary-40" />
            ) : (
                <Unpined className="text-neutral-60" />
            )}
        </button>
    );
}

interface TokenRowButtonProps {
    coinBalance: CoinBalanceType;
    children: ReactNode;
    to: string;
    onClick?: () => void;
}

function TokenRowButton({ coinBalance, children, to, onClick }: TokenRowButtonProps) {
    return (
        <ButtonOrLink
            to={to}
            key={coinBalance.coinType}
            onClick={onClick}
            className="text-steel hover:text-hero text-subtitle font-medium no-underline hover:font-semibold"
        >
            {children}
        </ButtonOrLink>
    );
}

interface TokenRowProps {
    coinBalance: CoinBalanceType;
    renderActions?: boolean;
    onClick?: () => void;
}

export function TokenRow({ coinBalance, renderActions, onClick }: TokenRowProps) {
    const coinType = coinBalance.coinType;
    const balance = BigInt(coinBalance.totalBalance);
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(balance, coinType);
    const Tag = onClick ? 'button' : 'div';
    const params = new URLSearchParams({
        type: coinBalance.coinType,
    });
    const balanceInUsd = useBalanceInUSD(coinBalance.coinType, coinBalance.totalBalance);

    return (
        <Tag
            className={clsx(
                'hover:bg-iota/10 group flex items-center rounded border-transparent bg-transparent py-3 pl-1.5 pr-2',
                onClick && 'hover:cursor-pointer',
            )}
            onClick={onClick}
        >
            <div className="flex gap-2.5">
                <CoinIcon coinType={coinType} />
                <div className="flex flex-col items-start gap-1">
                    <Text variant="body" color="gray-90" weight="semibold" truncate>
                        {coinMeta?.name || symbol}
                    </Text>

                    {renderActions && (
                        <div className="group-hover:hidden">
                            <Text variant="subtitle" color="steel-dark" weight="medium">
                                {symbol}
                            </Text>
                        </div>
                    )}

                    {renderActions ? (
                        <div className="hidden items-center gap-2.5 group-hover:flex">
                            <TokenRowButton
                                coinBalance={coinBalance}
                                to={`/send?${params.toString()}`}
                                onClick={() =>
                                    ampli.selectedCoin({
                                        coinType: coinBalance.coinType,
                                        totalBalance: Number(formatted),
                                    })
                                }
                            >
                                Send
                            </TokenRowButton>
                        </div>
                    ) : (
                        <div className="flex items-center gap-1">
                            <Text variant="subtitleSmall" weight="semibold" color="gray-90">
                                {symbol}
                            </Text>
                            <Text variant="subtitleSmall" weight="medium" color="steel-dark">
                                {formatAddress(coinType)}
                            </Text>
                        </div>
                    )}
                </div>
            </div>

            <div className="ml-auto flex flex-col items-end gap-1">
                {balance > 0n && (
                    <Text variant="body" color="gray-90" weight="medium">
                        {formatted} {symbol}
                    </Text>
                )}

                {balanceInUsd && balanceInUsd > 0 && (
                    <Text variant="subtitle" color="steel-dark" weight="medium">
                        {Number(balanceInUsd).toLocaleString('en', {
                            style: 'currency',
                            currency: 'USD',
                        })}
                    </Text>
                )}
            </div>
        </Tag>
    );
}

interface MyTokensProps {
    coinBalances: CoinBalanceType[];
    isLoading: boolean;
    isFetched: boolean;
}

export function MyTokens({ coinBalances, isLoading, isFetched }: MyTokensProps) {
    const isDefiWalletEnabled = useIsWalletDefiEnabled();
    const network = useAppSelector(({ app }) => app.network);

    const [_pinned, { pinCoinType, unpinCoinType }] = usePinnedCoinTypes();

    const { recognized, pinned, unrecognized } = useSortedCoinsByCategories(coinBalances, _pinned);

    // Avoid perpetual loading state when fetching and retry keeps failing; add isFetched check.
    const isFirstTimeLoading = isLoading && !isFetched;

    return (
        <Loading loading={isFirstTimeLoading}>
            {recognized.length > 0 && (
                <TokenList title="My Coins" defaultOpen>
                    {recognized.map((coinBalance) =>
                        isDefiWalletEnabled ? (
                            <TokenRow
                                renderActions
                                key={coinBalance.coinType}
                                coinBalance={coinBalance}
                            />
                        ) : (
                            <TokenLink key={coinBalance.coinType} coinBalance={coinBalance} />
                        ),
                    )}
                </TokenList>
            )}

            {pinned.length > 0 && (
                <TokenList title="Pinned Coins" defaultOpen>
                    {pinned.map((coinBalance) => (
                        <TokenLink
                            key={coinBalance.coinType}
                            coinBalance={coinBalance}
                            clickableAction={
                                <PinButton
                                    isPinned
                                    onClick={() => {
                                        ampli.unpinnedCoin({ coinType: coinBalance.coinType });
                                        unpinCoinType(coinBalance.coinType);
                                    }}
                                />
                            }
                        />
                    ))}
                </TokenList>
            )}

            {unrecognized.length > 0 && (
                <TokenList
                    title={
                        unrecognized.length === 1
                            ? `${unrecognized.length} Unrecognized Coin`
                            : `${unrecognized.length} Unrecognized Coins`
                    }
                    defaultOpen={network !== Network.Mainnet}
                >
                    {unrecognized.map((coinBalance) => (
                        <TokenLink
                            key={coinBalance.coinType}
                            coinBalance={coinBalance}
                            clickableAction={
                                <PinButton
                                    onClick={() => {
                                        ampli.pinnedCoin({ coinType: coinBalance.coinType });
                                        pinCoinType(coinBalance.coinType);
                                    }}
                                />
                            }
                        />
                    ))}
                </TokenList>
            )}
        </Loading>
    );
}

function getMostNestedName(parsed: ReturnType<typeof parseStructTag>) {
    if (parsed.typeParams.length === 0) {
        return parsed.name;
    }

    if (typeof parsed.typeParams[0] === 'string') {
        return parsed.typeParams[0];
    }

    return getMostNestedName(parsed.typeParams[0]);
}

function getFallbackSymbol(coinType: string) {
    const parsed = parseStructTag(coinType);
    return getMostNestedName(parsed);
}

function TokenDetails({ coinType }: TokenDetailsProps) {
    const isDefiWalletEnabled = useIsWalletDefiEnabled();
    const [interstitialDismissed, setInterstitialDismissed] = useState<boolean>(false);
    const activeCoinType = coinType || IOTA_TYPE_ARG;
    const activeAccount = useActiveAccount();
    const activeAccountAddress = activeAccount?.address;
    const { data: domainName } = useResolveIotaNSName(activeAccountAddress);
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

    const { data: coinMetadata } = useCoinMetadata(activeCoinType);
    const coinSymbol = coinMetadata ? coinMetadata.symbol : getFallbackSymbol(activeCoinType);

    // Avoid perpetual loading state when fetching and retry keeps failing add isFetched check
    const isFirstTimeLoading = isPending && !isFetched;

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
    return (
        <>
            {isMainnet && data?.degraded && (
                <div className="border-warning-dark/20 bg-warning-light text-warning-dark mb-4 flex items-center rounded-2xl border border-solid px-3 py-2">
                    <Info12 className="shrink-0" />
                    <div className="ml-2">
                        <Text variant="pBodySmall" weight="medium">
                            We're sorry that the app is running slower than usual. We're working to
                            fix the issue and appreciate your patience.
                        </Text>
                    </div>
                </div>
            )}

            <Loading loading={isFirstTimeLoading}>
                {coinType && <PageTitle title={coinSymbol} back="/tokens" />}

                <div
                    className="flex h-full flex-1 flex-grow flex-col items-center gap-8"
                    data-testid="coin-page"
                >
                    <AccountsList />
                    <div className="flex w-full flex-col">
                        <PortfolioName
                            name={
                                activeAccount.nickname ??
                                domainName ??
                                formatAddress(activeAccountAddress)
                            }
                        />
                        {activeAccount.isLocked ? null : (
                            <>
                                <div
                                    data-testid="coin-balance"
                                    className={clsx(
                                        'mt-4 flex w-full flex-col items-center gap-3 rounded-2xl px-4 py-5',
                                        isDefiWalletEnabled
                                            ? 'bg-gradients-graph-cards'
                                            : 'bg-hero/5',
                                    )}
                                >
                                    <div className="flex flex-col items-center gap-1">
                                        <CoinBalance amount={tokenBalance} type={activeCoinType} />
                                    </div>

                                    {!accountHasIota ? (
                                        <div className="flex flex-col gap-5">
                                            <div className="flex flex-col flex-nowrap items-center justify-center px-2.5 text-center">
                                                <Text
                                                    variant="pBodySmall"
                                                    color="gray-80"
                                                    weight="normal"
                                                >
                                                    {isMainnet
                                                        ? 'Buy IOTA to get started'
                                                        : 'To send transactions on the Iota network, you need IOTA in your wallet.'}
                                                </Text>
                                            </div>
                                            <FaucetRequestButton />
                                        </div>
                                    ) : null}
                                    {isError ? (
                                        <Alert>
                                            <div>
                                                <strong>Error updating balance</strong>
                                            </div>
                                        </Alert>
                                    ) : null}
                                    <div className="grid w-full grid-cols-3 gap-3">
                                        <LargeButton
                                            center
                                            data-testid="send-coin-button"
                                            to={`/send${
                                                coinBalance?.coinType
                                                    ? `?${new URLSearchParams({
                                                          type: coinBalance.coinType,
                                                      }).toString()}`
                                                    : ''
                                            }`}
                                            disabled={!tokenBalance}
                                        >
                                            Send
                                        </LargeButton>

                                        {!accountHasIota && (
                                            <LargeButton disabled to="/stake" center>
                                                Stake
                                            </LargeButton>
                                        )}
                                    </div>

                                    <div className="w-full">
                                        {accountHasIota || delegatedStake?.length ? (
                                            <TokenIconLink
                                                disabled={!tokenBalance}
                                                accountAddress={activeAccountAddress}
                                            />
                                        ) : null}
                                    </div>
                                </div>
                            </>
                        )}
                    </div>
                    {activeAccount.isLocked ? (
                        <UnlockAccountButton account={activeAccount} />
                    ) : (
                        <MyTokens
                            coinBalances={coinBalances ?? []}
                            isLoading={coinBalancesLoading}
                            isFetched={coinBalancesFetched}
                        />
                    )}
                </div>
            </Loading>
        </>
    );
}

export default TokenDetails;
