// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useState } from 'react';
import { Theme, toast, useTheme } from '@iota/core';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import AddProfileImage from '_assets/images/balance_finder_intro.png';
import AddProfileImageDark from '_assets/images/balance_finder_intro_darkmode.png';
import {
    Card,
    CardType,
    CardImage,
    CardBody,
    CardAction,
    ImageType,
    CardActionType,
    Button,
    ButtonType,
    ImageShape,
} from '@iota/apps-ui-kit';
import {
    AccountsFormType,
    ConnectLedgerModal,
    PageTemplate,
    useBootstrapSourceFlow,
    useSourceFlow,
} from '_components';
import { getLedgerConnectionErrorMessage } from '../../helpers/errorMessages';
import { useAppSelector, useCheckCameraPermissionStatus, useAccounts } from '_hooks';
import { Create, Ledger, Keystone, Wallet } from '@iota/apps-ui-icons';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import Browser from 'webextension-polyfill';
import clsx from 'clsx';
import { isFirstAccount } from '../../helpers';
import { ACCOUNT_FORM_TYPE_TO_AMPLI } from '_src/shared/analytics';

export interface ActionCardItem {
    title: string;
    subtitle?: string;
    icon: React.ComponentType<{ className: string }>;
    actionType: AccountsFormType;
}
export interface CardLinkItem {
    title: string;
    subtitle?: string;
    icon: React.ComponentType<{ className: string }>;
    href: string;
}

export interface ButtonCardItem {
    title: string;
    icon: React.ComponentType<{ className: string }>;
    actionType: AccountsFormType;
}

async function openTabWithSearchParams(params: Record<string, string>) {
    const currentURL = new URL(window.location.href);
    const [currentHash, currentHashSearch] = currentURL.hash.split('?');
    const urlSearchParams = new URLSearchParams(currentHashSearch);
    for (const [key, value] of Object.entries(params)) {
        urlSearchParams.set(key, value);
    }
    currentURL.hash = `${currentHash}?${urlSearchParams.toString()}`;
    currentURL.searchParams.delete('type');
    await Browser.tabs.create({
        url: currentURL.href,
    });
}

async function openTabOnImportKeystone(sourceFlow: string) {
    const url = new URL(Browser.runtime.getURL('ui.html'));
    url.hash = `/accounts/import-keystone?sourceFlow=${encodeURIComponent(sourceFlow)}`;
    await Browser.tabs.create({
        url: url.href,
    });
}

export function AddAccountPage() {
    const { theme } = useTheme();
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const forceShowLedger =
        searchParams.has('showLedger') && searchParams.get('showLedger') !== 'false';
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(forceShowLedger);
    const { sourceFlowRef } = useSourceFlow();
    useBootstrapSourceFlow();

    const sourceFlow = sourceFlowRef.current;
    const isPopupOrSidePanel = useAppSelector(
        (state) =>
            state.app.extensionViewType === ExtensionViewType.Popup ||
            state.app.extensionViewType === ExtensionViewType.SidePanel,
    );
    const [cameraPermissionStatus] = useCheckCameraPermissionStatus();
    const { data: accounts } = useAccounts();

    const cardLinks: CardLinkItem[] = [
        {
            title: 'Create a new wallet',
            icon: Create,
            subtitle: 'Mnemonic or Passkey',
            href: '/accounts/create-new',
        },
        {
            title: 'Add existing wallet',
            icon: Wallet,
            subtitle: 'Import or restore',
            href: '/accounts/import-existing',
        },
    ];

    const hardwareWalletOptions = [
        {
            title: 'Ledger',
            icon: Ledger,
            actionType: AccountsFormType.ImportLedger,
        },
        {
            title: 'Keystone',
            icon: Keystone,
            actionType: AccountsFormType.ImportKeystone,
        },
    ] as const satisfies ButtonCardItem[];

    async function handleCardAction(
        actionType: (typeof hardwareWalletOptions)[number]['actionType'],
    ) {
        const ampliData = ACCOUNT_FORM_TYPE_TO_AMPLI[actionType];

        if (ampliData) {
            ampli.clickedCreateNewAccount({
                accountType: ampliData.accountType,
                accountOrigin: ampliData.accountOrigin,
                isFirstAccount: isFirstAccount(accounts),
                sourceFlow,
            });
        }

        switch (actionType) {
            case AccountsFormType.ImportLedger:
                if (isPopupOrSidePanel) {
                    await openTabWithSearchParams({
                        showLedger: 'true',
                        sourceFlow,
                    });
                    window.close();
                } else {
                    setConnectLedgerModalOpen(true);
                }
                break;
            case AccountsFormType.ImportKeystone:
                if (isPopupOrSidePanel && cameraPermissionStatus === 'prompt') {
                    await openTabOnImportKeystone(sourceFlow);
                    window.close();
                } else {
                    navigate('/accounts/import-keystone');
                }
                break;
            default:
                throw new Error('Unsupported action type');
        }
    }

    return (
        <PageTemplate
            title="Add Profile"
            isTitleCentered
            onClose={() => navigate('/')}
            showBackButton
            onBack={() => navigate('/')}
        >
            <div className="flex h-full w-full flex-col">
                <div className="flex w-full flex-1 flex-col pb-md--rs text-center">
                    <img
                        src={theme === Theme.Dark ? AddProfileImageDark : AddProfileImage}
                        alt="Add Profile"
                        height={187}
                        className="mx-auto aspect-[4/3] max-h-[187px] w-auto object-cover"
                    />

                    <div className="flex flex-1 flex-col items-center gap-xxs">
                        <h2 className="text-headline-sm font-medium leading-[120%] tracking-[-0.4px] text-iota-neutral-10 dark:text-iota-neutral-92">
                            Your journey into Web3
                        </h2>

                        <p className="text-body-lg font-normal text-iota-neutral-10 dark:text-iota-neutral-92">
                            Access the fast, secure, and scalable future of Web3.
                        </p>
                    </div>
                </div>

                <div className="flex flex-col gap-lg">
                    <div className="flex flex-col gap-y-xs text-start">
                        {cardLinks.map((card) => (
                            <Link to={card.href} key={card.title} className="no-underline">
                                <Card key={card.title} type={CardType.Filled} isHoverable>
                                    <OnboardingCardIcon Icon={card.icon} />
                                    <CardBody title={card.title} subtitle={card.subtitle} />
                                    <CardAction type={CardActionType.Link} />
                                </Card>
                            </Link>
                        ))}
                    </div>

                    <div className="flex flex-col gap-xs text-center">
                        <span className="text-label-lg font-medium text-iota-neutral-60 dark:text-iota-neutral-40">
                            Hardware wallets
                        </span>
                        <div className="grid grid-cols-2 gap-2">
                            {hardwareWalletOptions.map((card) => (
                                <Button
                                    key={card.title}
                                    type={ButtonType.Secondary}
                                    onClick={() => handleCardAction(card.actionType)}
                                    text={card.title}
                                    icon={
                                        <card.icon className="h-5 w-5 text-iota-primary-30 dark:text-iota-primary-80" />
                                    }
                                />
                            ))}
                        </div>
                    </div>
                </div>
            </div>

            {isConnectLedgerModalOpen && (
                <ConnectLedgerModal
                    onClose={() => {
                        setConnectLedgerModalOpen(false);
                    }}
                    onError={(error) => {
                        setConnectLedgerModalOpen(false);
                        toast.error(
                            getLedgerConnectionErrorMessage(error) || 'Something went wrong.',
                        );
                    }}
                    onConfirm={() => {
                        navigate('/accounts/import-ledger-accounts');
                    }}
                    requestLedgerPermissionsFirst
                />
            )}
        </PageTemplate>
    );
}

interface CardIconProps {
    Icon: React.ComponentType<{ className: string }>;
    isLegacy?: boolean;
}

const DEFAULT_BG_COLOR = 'dark:bg-iota-primary-10 bg-iota-primary-90';
const DEFAULT_ICON_COLOR = 'text-iota-primary-20 dark:text-iota-primary-90';
const LEGACY_BG_COLOR = 'dark:bg-iota-warning-10 bg-iota-warning-90';
const LEGACY_ICON_COLOR = 'text-iota-warning-20 dark:text-iota-warning-90';

export function OnboardingCardIcon({ Icon, isLegacy: isLegacy = false }: CardIconProps) {
    const bgColor = isLegacy ? LEGACY_BG_COLOR : DEFAULT_BG_COLOR;
    const iconColor = isLegacy ? LEGACY_ICON_COLOR : DEFAULT_ICON_COLOR;

    return (
        <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
            <div className={clsx(bgColor, ' rounded-lg p-[10px]')}>
                <Icon className={clsx(iconColor, 'h-5 w-5')} />
            </div>
        </CardImage>
    );
}
