// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
    AccountsFormType,
    useAccountsFormContext,
    LedgerAccountList,
    type SelectableLedgerAccount,
    useDeriveLedgerAccounts,
    type DerivedLedgerAccount,
    Overlay,
} from '_components';
import { getIotaApplicationErrorMessage } from '../../helpers/errorMessages';
import { useAccounts } from '../../hooks/useAccounts';
import { Button } from '@iota/apps-ui-kit';
import { CheckmarkFilled, Loader } from '@iota/ui-icons';

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
    const isUnlockButtonDisabled = numSelectedAccounts === 0;
    const [, setAccountsFormValues] = useAccountsFormContext();

    let importLedgerAccountsBody: JSX.Element | null = null;
    if (areLedgerAccountsLoading) {
        importLedgerAccountsBody = <LedgerViewLoading />;
    } else if (areAllAccountsImported) {
        importLedgerAccountsBody = <LedgerViewAllAccountsImported />;
    } else if (!encounteredDerviceAccountsError) {
        const selectedLedgerAddresses = selectedLedgerAccounts.map(({ address }) => address);
        importLedgerAccountsBody = (
            <div className="max-h-[530px] w-full overflow-auto">
                <LedgerAccountList
                    accounts={ledgerAccounts.map((ledgerAccount) => ({
                        ...ledgerAccount,
                        isSelected: selectedLedgerAddresses.includes(ledgerAccount.address),
                    }))}
                    onAccountClick={onAccountClick}
                    selectAll={selectAllAccounts}
                />
            </div>
        );
    }

    function selectAllAccounts() {
        if (ledgerAccounts) {
            setSelectedLedgerAccounts(ledgerAccounts);
        }
    }

    function handleNextClick() {
        setAccountsFormValues({
            type: AccountsFormType.ImportLedger,
            accounts: selectedLedgerAccounts.map(({ address, derivationPath, publicKey }) => ({
                address,
                derivationPath,
                publicKey: publicKey!,
            })),
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportLedger,
                successRedirect,
            }).toString()}`,
        );
    }

    return (
        <Overlay
            showModal
            title="Import Wallets"
            closeOverlay={() => {
                navigate(-1);
            }}
            titleCentered={false}
        >
            <div className="flex h-full w-full flex-col">
                {importLedgerAccountsBody}
                <div className="flex flex-1 items-end">
                    <Button
                        text="Next"
                        disabled={isUnlockButtonDisabled}
                        onClick={handleNextClick}
                        fullWidth
                    />
                </div>
            </div>
        </Overlay>
    );
}

function LedgerViewLoading() {
    return (
        <div className="flex h-full w-full flex-row items-center justify-center gap-x-sm">
            <Loader className="h-5 w-5 animate-spin text-primary-30" />
            <span className="text-title-lg text-neutral-10">Looking for Accounts...</span>
        </div>
    );
}

function LedgerViewAllAccountsImported() {
    return (
        <div className="flex h-full w-full flex-row items-center justify-center gap-x-sm [&_svg]:h-6 [&_svg]:w-6">
            <CheckmarkFilled className="text-primary-30" />
            <span className="text-title-lg text-neutral-10">Imported all Ledger Accounts</span>
        </div>
    );
}
