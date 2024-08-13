// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useState } from 'react';
import toast from 'react-hot-toast';
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
} from '../../components/accounts/AccountsFormContext';
import { ConnectLedgerModal } from '../../components/ledger/ConnectLedgerModal';
import { getLedgerConnectionErrorMessage } from '../../helpers/errorMessages';
import { useAppSelector } from '../../hooks';
import { useCreateAccountsMutation } from '../../hooks/useCreateAccountMutation';
import { AppType } from '../../redux/slices/app/AppType';
import { Create, ImportPass, Key, Seed, Ledger } from '@iota/ui-icons';
import { PageTemplate } from '../../components/PageTemplate';

enum ActionType {
    CreateNew = 'CreateNew',
    ImportPassphrase = 'ImportPassphrase',
    ImportPrivateKey = 'ImportPrivateKey',
    ImportSeed = 'ImportSeed',
    ImportLedger = 'ImportLedger',
}

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
                    actionType: ActionType.CreateNew,
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
                    actionType: ActionType.ImportPassphrase,
                    isDisabled: createAccountsMutation.isPending,
                },
                {
                    title: 'Private Key',
                    icon: Key,
                    actionType: ActionType.ImportPrivateKey,
                    isDisabled: createAccountsMutation.isPending,
                },
                {
                    title: 'Seed',
                    icon: Seed,
                    actionType: ActionType.ImportSeed,
                    isDisabled: createAccountsMutation.isPending,
                },
            ],
        },
        {
            title: 'Import from Legder',
            cards: [
                {
                    title: 'Ledger',
                    icon: Ledger,
                    actionType: ActionType.ImportLedger,
                    isDisabled: createAccountsMutation.isPending,
                },
            ],
        },
    ];

    const handleCardAction = async (actionType: ActionType) => {
        switch (actionType) {
            case ActionType.CreateNew:
                setAccountsFormValues({ type: AccountsFormType.NewMnemonic });
                ampli.clickedCreateNewAccount({ sourceFlow });
                navigate(
                    `/accounts/protect-account?accountsFormType=${AccountsFormType.NewMnemonic}`,
                );
                break;
            case ActionType.ImportPassphrase:
                ampli.clickedImportPassphrase({ sourceFlow });
                navigate('/accounts/import-passphrase');
                break;
            case ActionType.ImportPrivateKey:
                ampli.clickedImportPrivateKey({ sourceFlow });
                navigate('/accounts/import-private-key');
                break;
            case ActionType.ImportSeed:
                navigate('/accounts/import-seed');
                break;
            case ActionType.ImportLedger:
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
                        <span className="text-label-lg text-neutral-60">{group.title}</span>
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
        <Icon className="h-5 w-5 text-primary-30" />
    </CardImage>
);
