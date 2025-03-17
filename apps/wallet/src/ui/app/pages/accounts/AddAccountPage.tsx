// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useState } from 'react';
import { toast } from '@iota/core';
import { useNavigate, useSearchParams } from 'react-router-dom';
import Browser from 'webextension-polyfill';
import {
    Card,
    CardType,
    CardImage,
    CardBody,
    CardAction,
    ImageType,
    CardActionType,
} from '@iota/apps-ui-kit';
import {
    AccountsFormType,
    useAccountsFormContext,
    ConnectLedgerModal,
    PageTemplate,
} from '_components';
import { getLedgerConnectionErrorMessage } from '../../helpers/errorMessages';
import { useAppSelector, useCreateAccountsMutation } from '_hooks';
import { AppType } from '../../redux/slices/app/appType';
import { Create, ImportPass, Key, Seed, Ledger } from '@iota/apps-ui-icons';

async function openTabWithSearchParam(searchParam: string, searchParamValue: string) {
    const currentURL = new URL(window.location.href);
    const [currentHash, currentHashSearch] = currentURL.hash.split('?');
    const urlSearchParams = new URLSearchParams(currentHashSearch);
    urlSearchParams.set(searchParam, searchParamValue);
    currentURL.hash = `${currentHash}?${urlSearchParams.toString()}`;
    currentURL.searchParams.delete('type');
    await Browser.tabs.create({
        url: currentURL.href,
    });
}

export function AddAccountPage() {
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const sourceFlow = searchParams.get('sourceFlow') || 'Unknown';
    const forceShowLedger =
        searchParams.has('showLedger') && searchParams.get('showLedger') !== 'false';
    const [, setAccountsFormValues] = useAccountsFormContext();
    const isPopup = useAppSelector((state) => state.app.appType === AppType.Popup);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(forceShowLedger);
    const createAccountsMutation = useCreateAccountsMutation();
    const cardGroups = [
        {
            title: 'Create a new mnemonic profile',
            cards: [
                {
                    title: 'Create New',
                    icon: Create,
                    actionType: AccountsFormType.NewMnemonic,
                    isDisabled: createAccountsMutation.isPending,
                },
            ],
        },
        {
            title: 'Import',
            cards: [
                {
                    title: 'Mnemonic',
                    icon: ImportPass,
                    actionType: AccountsFormType.ImportMnemonic,
                    isDisabled: createAccountsMutation.isPending,
                },
                {
                    title: 'Private Key',
                    icon: Key,
                    actionType: AccountsFormType.ImportPrivateKey,
                    isDisabled: createAccountsMutation.isPending,
                },
                {
                    title: 'Seed',
                    icon: Seed,
                    actionType: AccountsFormType.ImportSeed,
                    isDisabled: createAccountsMutation.isPending,
                },
            ],
        },
        {
            title: 'Import from Ledger',
            cards: [
                {
                    title: 'Ledger',
                    icon: Ledger,
                    actionType: AccountsFormType.ImportLedger,
                    isDisabled: createAccountsMutation.isPending,
                },
            ],
        },
    ];

    const handleCardAction = async (actionType: AccountsFormType) => {
        switch (actionType) {
            case AccountsFormType.NewMnemonic:
                setAccountsFormValues({ type: AccountsFormType.NewMnemonic });
                ampli.clickedCreateNewAccount({ sourceFlow });
                navigate(
                    `/accounts/protect-account?accountsFormType=${AccountsFormType.NewMnemonic}`,
                );
                break;
            case AccountsFormType.ImportMnemonic:
                ampli.clickedImportPassphrase({ sourceFlow });
                navigate('/accounts/import-passphrase');
                break;
            case AccountsFormType.ImportPrivateKey:
                ampli.clickedImportPrivateKey({ sourceFlow });
                navigate('/accounts/import-private-key');
                break;
            case AccountsFormType.ImportSeed:
                navigate('/accounts/import-seed');
                break;
            case AccountsFormType.ImportLedger:
                ampli.openedConnectLedgerFlow({ sourceFlow });
                if (isPopup) {
                    await openTabWithSearchParam('showLedger', 'true');
                    window.close();
                } else {
                    setConnectLedgerModalOpen(true);
                }
                break;
            default:
                break;
        }
    };

    return (
        <PageTemplate
            title="Add Profile"
            isTitleCentered
            onClose={() => navigate('/')}
            showBackButton
        >
            <div className="flex h-full w-full flex-col gap-4 ">
                {cardGroups.map((group, groupIndex) => (
                    <div key={groupIndex} className="flex flex-col gap-y-2">
                        <span className="text-label-lg text-neutral-60 dark:text-neutral-40">
                            {group.title}
                        </span>
                        {group.cards.map((card, cardIndex) => (
                            <Card
                                key={cardIndex}
                                type={CardType.Filled}
                                onClick={() => handleCardAction(card.actionType)}
                                isDisabled={card.isDisabled}
                            >
                                <CardIcon Icon={card.icon} />
                                <CardBody title={card.title} />
                                <CardAction type={CardActionType.Link} />
                            </Card>
                        ))}
                    </div>
                ))}
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
                        ampli.connectedHardwareWallet({ hardwareWalletType: 'Ledger' });
                        navigate('/accounts/import-ledger-accounts');
                    }}
                />
            )}
        </PageTemplate>
    );
}

const CardIcon = ({ Icon }: { Icon: React.ComponentType<{ className: string }> }) => (
    <CardImage type={ImageType.BgTransparent}>
        <Icon className="h-5 w-5 text-primary-30 dark:text-primary-80" />
    </CardImage>
);
