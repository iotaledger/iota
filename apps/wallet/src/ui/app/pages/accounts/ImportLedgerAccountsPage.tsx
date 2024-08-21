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
import { CheckmarkFilled } from '@iota/ui-icons';

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
        importLedgerAccountsBody = <LedgerViewWhenLoading />;
    } else if (areAllAccountsImported) {
        importLedgerAccountsBody = <LedgerViewWhenAllAccountsImported />;
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

function LedgerViewWhenLoading() {
    return (
        <div className="flex h-full w-full flex-row items-center justify-center gap-x-sm">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                className="h-5 w-5 animate-spin"
            >
                <path
                    fill-rule="evenodd"
                    clip-rule="evenodd"
                    d="M5.53285 9.32121C5.00303 10.6003 4.86441 12.0078 5.13451 13.3656C5.4046 14.7235 6.07129 15.9708 7.05026 16.9497C8.02922 17.9287 9.2765 18.5954 10.6344 18.8655C11.9922 19.1356 13.3997 18.997 14.6788 18.4672C15.9579 17.9373 17.0511 17.0401 17.8203 15.889C18.5895 14.7378 19 13.3845 19 12C19 11.4477 19.4477 11 20 11C20.5523 11 21 11.4477 21 12C21 13.78 20.4722 15.5201 19.4832 17.0001C18.4943 18.4802 17.0887 19.6337 15.4442 20.3149C13.7996 20.9961 11.99 21.1743 10.2442 20.8271C8.49836 20.4798 6.89471 19.6226 5.63604 18.364C4.37737 17.1053 3.5202 15.5016 3.17294 13.7558C2.82567 12.01 3.0039 10.2004 3.68509 8.55585C4.36628 6.91131 5.51983 5.50571 6.99987 4.51677C8.47991 3.52784 10.22 3 12 3C12.5523 3 13 3.44772 13 4C13 4.55228 12.5523 5 12 5C10.6155 5 9.26215 5.41054 8.11101 6.17971C6.95987 6.94888 6.06266 8.04213 5.53285 9.32121Z"
                    fill="#3131FF"
                />
            </svg>
            <span className="text-title-lg text-neutral-10">Looking for Accounts...</span>
        </div>
    );
}

function LedgerViewWhenAllAccountsImported() {
    return (
        <div className="flex h-full w-full flex-row items-center justify-center gap-x-sm [&_svg]:h-6 [&_svg]:w-6">
            <CheckmarkFilled className="text-primary-30" />
            <span className="text-title-lg text-neutral-10">Imported all Ledge Accounts</span>
        </div>
    );
}
