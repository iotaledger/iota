// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_src/ui/app/shared/ButtonUI';
import { Link } from '_src/ui/app/shared/Link';
import { Text } from '_src/ui/app/shared/text';
import {
    Spinner16 as SpinnerIcon,
    ThumbUpStroke32 as ThumbUpIcon,
    LockUnlocked16 as UnlockedLockIcon,
} from '@iota/icons';
import { useCallback, useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useNavigate, useSearchParams } from 'react-router-dom';

import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import {
    LedgerAccountList,
    type SelectableLedgerAccount,
} from '../../components/ledger/LedgerAccountList';
import {
    useDeriveLedgerAccounts,
    type DerivedLedgerAccount,
} from '../../components/ledger/useDeriveLedgerAccounts';
import Overlay from '../../components/overlay';
import { getIotaApplicationErrorMessage } from '../../helpers/errorMessages';
import { useAccounts } from '../../hooks/useAccounts';

const NUM_LEDGER_ACCOUNTS_TO_DERIVE_BY_DEFAULT = 10;

export function ImportLedgerAccountsPage() {
    const [searchParams] = useSearchParams();
    const successRedirect = searchParams.get('successRedirect') || '/tokens';
    const navigate = useNavigate();
    const { data: existingAccounts } = useAccounts();
    const [selectedLedgerAccounts, setSelectedLedgerAccounts] = useState<DerivedLedgerAccount[]>(
        [],
    );
    const {
        data: ledgerAccounts,
        error: ledgerError,
        isPending: areLedgerAccountsLoading,
        isError: encounteredDerviceAccountsError,
    } = useDeriveLedgerAccounts({
        numAccountsToDerive: NUM_LEDGER_ACCOUNTS_TO_DERIVE_BY_DEFAULT,
        select: (ledgerAccounts) => {
            return ledgerAccounts.filter(
                ({ address }) => !existingAccounts?.some((account) => account.address === address),
            );
        },
    });

    useEffect(() => {
        if (ledgerError) {
            toast.error(getIotaApplicationErrorMessage(ledgerError) || 'Something went wrong.');
            navigate(-1);
        }
    }, [ledgerError, navigate]);

    const onAccountClick = useCallback(
        (targetAccount: SelectableLedgerAccount) => {
            if (targetAccount.isSelected) {
                setSelectedLedgerAccounts((prevState) =>
                    prevState.filter((ledgerAccount) => {
                        return ledgerAccount.address !== targetAccount.address;
                    }),
                );
            } else {
                setSelectedLedgerAccounts((prevState) => [...prevState, targetAccount]);
            }
        },
        [setSelectedLedgerAccounts],
    );
    const numImportableAccounts = ledgerAccounts?.length;
    const numSelectedAccounts = selectedLedgerAccounts.length;
    const areAllAccountsImported = numImportableAccounts === 0;
    const areAllAccountsSelected = numSelectedAccounts === numImportableAccounts;
    const isUnlockButtonDisabled = numSelectedAccounts === 0;
    const isSelectAllButtonDisabled = areAllAccountsImported || areAllAccountsSelected;
    const [, setAccountsFormValues] = useAccountsFormContext();

    let summaryCardBody: JSX.Element | null = null;
    if (areLedgerAccountsLoading) {
        summaryCardBody = (
            <div className="flex h-full w-full flex-col items-center justify-center gap-2">
                <SpinnerIcon className="text-steel h-4 w-4 animate-spin" />
                <Text variant="pBodySmall" color="steel-darker">
                    Looking for accounts
                </Text>
            </div>
        );
    } else if (areAllAccountsImported) {
        summaryCardBody = (
            <div className="flex h-full w-full flex-col items-center justify-center gap-2">
                <ThumbUpIcon className="text-steel h-8 w-8" />
                <Text variant="pBodySmall" color="steel-darker">
                    All Ledger accounts have been imported.
                </Text>
            </div>
        );
    } else if (!encounteredDerviceAccountsError) {
        const selectedLedgerAddresses = selectedLedgerAccounts.map(({ address }) => address);
        summaryCardBody = (
            <div className="custom-scrollbar -mr-2 mt-1 max-h-[272px] overflow-auto pr-2">
                <LedgerAccountList
                    accounts={ledgerAccounts.map((ledgerAccount) => ({
                        ...ledgerAccount,
                        isSelected: selectedLedgerAddresses.includes(ledgerAccount.address),
                    }))}
                    onAccountClick={onAccountClick}
                />
            </div>
        );
    }

    return (
        <Overlay
            showModal
            title="Import Accounts"
            closeOverlay={() => {
                navigate(-1);
            }}
        >
            <div className="flex h-full w-full flex-col gap-5">
                <div className="border-gray-45 flex h-full max-h-[368px] flex-col rounded-2xl border border-solid bg-white">
                    <div className="bg-gray-40 rounded-t-2xl py-2.5 text-center">
                        <Text variant="captionSmall" weight="bold" color="steel-darker" truncate>
                            {areAllAccountsImported
                                ? 'Ledger Accounts '
                                : 'Connect Ledger Accounts'}
                        </Text>
                    </div>
                    <div className="grow px-4 py-2">{summaryCardBody}</div>
                    <div className="border-gray-40 w-full rounded-b-2xl border-x-0 border-b-0 border-t border-solid pb-4 pt-3 text-center">
                        <div className="ml-auto mr-auto w-fit">
                            <Link
                                text="Select All Accounts"
                                color="heroDark"
                                weight="medium"
                                onClick={() => {
                                    if (ledgerAccounts) {
                                        setSelectedLedgerAccounts(ledgerAccounts);
                                    }
                                }}
                                disabled={isSelectAllButtonDisabled}
                            />
                        </div>
                    </div>
                </div>
                <div className="flex flex-1 items-end">
                    <Button
                        variant="primary"
                        size="tall"
                        before={<UnlockedLockIcon />}
                        text="Next"
                        disabled={isUnlockButtonDisabled}
                        onClick={() => {
                            setAccountsFormValues({
                                type: AccountsFormType.ImportLedger,
                                accounts: selectedLedgerAccounts.map(
                                    ({ address, derivationPath, publicKey }) => ({
                                        address,
                                        derivationPath,
                                        publicKey: publicKey!,
                                    }),
                                ),
                            });
                            navigate(
                                `/accounts/protect-account?${new URLSearchParams({
                                    accountsFormType: AccountsFormType.ImportLedger,
                                    successRedirect,
                                }).toString()}`,
                            );
                        }}
                    />
                </div>
            </div>
        </Overlay>
    );
}
