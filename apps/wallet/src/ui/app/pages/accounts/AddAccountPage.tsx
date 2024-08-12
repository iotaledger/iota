// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useState } from 'react';
import toast from 'react-hot-toast';
import { useNavigate, useSearchParams } from 'react-router-dom';
import Browser from 'webextension-polyfill';
import {
    Header,
    Card,
    CardType,
    CardImage,
    CardBody,
    CardActionType,
    CardAction,
    ImageType,
} from '@iota/apps-ui-kit';
import { AccountsFormType, useAccountsFormContext } from '_components/accounts/AccountsFormContext';
import { ConnectLedgerModal } from '_components/ledger/ConnectLedgerModal';
import { getLedgerConnectionErrorMessage } from '../../helpers/errorMessages';
import { useAppSelector } from '../../hooks';
import { useCreateAccountsMutation } from '../../hooks/useCreateAccountMutation';
import { AppType } from '../../redux/slices/app/AppType';
import { Create, ImportPass, Key, Seed, Ledger } from '@iota/ui-icons';

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

    return (
        <div className="h-full w-full">
            <Header
                title="Add Profile"
                titleCentered
                onClose={() => navigate('/')}
                onBack={() => navigate('/')}
            />
            <div className="flex h-full w-full flex-col gap-4 bg-white p-md">
                <div className="flex flex-col gap-y-4">
                    <div className="flex flex-col gap-y-2">
                        <span className="text-label-lg text-neutral-60">
                            Create a new mnemonic profile
                        </span>
                        <Card
                            type={CardType.Filled}
                            onClick={() => {
                                setAccountsFormValues({ type: AccountsFormType.NewMnemonic });
                                ampli.clickedCreateNewAccount({ sourceFlow });
                                navigate(
                                    `/accounts/protect-account?accountsFormType=${AccountsFormType.NewMnemonic}`,
                                );
                            }}
                            isDisabled={createAccountsMutation.isPending}
                        >
                            <CardIcon Icon={Create} />
                            <CardBody title="Create New" />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    </div>
                    <div className="flex flex-col gap-y-2">
                        <span className="text-label-lg text-neutral-60">Import</span>
                        <Card
                            type={CardType.Filled}
                            onClick={() => {
                                ampli.clickedImportPassphrase({ sourceFlow });
                                navigate('/accounts/import-passphrase');
                            }}
                            isDisabled={createAccountsMutation.isPending}
                        >
                            <CardIcon Icon={ImportPass} />
                            <CardBody title="Mnemonic" />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                        <Card
                            type={CardType.Filled}
                            onClick={() => {
                                ampli.clickedImportPrivateKey({ sourceFlow });
                                navigate('/accounts/import-private-key');
                            }}
                            isDisabled={createAccountsMutation.isPending}
                        >
                            <CardIcon Icon={Key} />
                            <CardBody title="Private Key" />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                        <Card
                            type={CardType.Filled}
                            onClick={() => {
                                navigate('/accounts/import-seed');
                            }}
                            isDisabled={createAccountsMutation.isPending}
                        >
                            <CardIcon Icon={Seed} />
                            <CardBody title="Seed" />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    </div>
                    <div className="flex flex-col gap-y-2">
                        <span className="text-label-lg text-neutral-60">Import from Legder</span>
                        <Card
                            type={CardType.Filled}
                            onClick={async () => {
                                ampli.openedConnectLedgerFlow({ sourceFlow });
                                if (isPopup) {
                                    await openTabWithSearchParam('showLedger', 'true');
                                    window.close();
                                } else {
                                    setConnectLedgerModalOpen(true);
                                }
                            }}
                            isDisabled={createAccountsMutation.isPending}
                        >
                            <CardIcon Icon={Ledger} />
                            <CardBody title="Ledger" />
                            <CardAction type={CardActionType.Link} />
                        </Card>
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
                        ampli.connectedHardwareWallet({ hardwareWalletType: 'Ledger' });
                        navigate('/accounts/import-ledger-accounts');
                    }}
                />
            )}
        </div>
    );
}

const CardIcon = ({ Icon }: { Icon: React.ComponentType<{ className: string }> }) => (
    <CardImage type={ImageType.BgTransparent}>
        <Icon className="h-5 w-5 text-primary-30" />
    </CardImage>
);
