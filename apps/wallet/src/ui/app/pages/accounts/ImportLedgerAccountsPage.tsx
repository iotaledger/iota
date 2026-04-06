// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';
import { toast } from '@iota/core';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
    AccountsFormType,
    useAccountsFormContext,
    AccountList,
    useDeriveLedgerAccounts,
    type DerivedLedgerAccount,
    Overlay,
} from '_components';
import { useAccounts } from '_hooks';
import { getIotaApplicationErrorMessage } from '../../helpers/errorMessages';
import { Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';

const LEDGER_ACCOUNTS_DERIVE_CHUNKS_SIZE = 10;

export function ImportLedgerAccountsPage() {
    const [searchParams] = useSearchParams();
    const successRedirect = searchParams.get('successRedirect') || '/tokens';
    const navigate = useNavigate();
    const [selectedLedgerAccounts, setSelectedLedgerAccounts] = useState<Set<string>>(new Set());
    const {
        mainPublicKey: { data: mainPublicKey, isFetching: isLoadingMainPublicKey },
        accounts,
        advance: {
            error: ledgerError,
            isPending: areLedgerAccountsLoading,
            isError: encounteredDerviceAccountsError,
            mutateAsync: loadMore,
        },
    } = useDeriveLedgerAccounts({
        chunkSize: LEDGER_ACCOUNTS_DERIVE_CHUNKS_SIZE,
    });

    useEffect(() => {
        if (ledgerError) {
            toast.error(getIotaApplicationErrorMessage(ledgerError) || 'Something went wrong.');
            navigate(-1);
        }
    }, [ledgerError, navigate]);

    const onAccountClick = useCallback(
        (targetAccount: DerivedLedgerAccount, checked: boolean) => {
            setSelectedLedgerAccounts((accounts) => {
                if (checked) {
                    accounts.add(targetAccount.address);
                } else {
                    accounts.delete(targetAccount.address);
                }

                return new Set(accounts);
            });
        },
        [setSelectedLedgerAccounts],
    );

    const { data: existingAccounts } = useAccounts();
    const existingAddresses = new Set((existingAccounts ?? []).map((acc) => acc.address));

    const importableAccounts = accounts?.filter((acc) => !existingAddresses.has(acc.address));
    const numImportableAccounts = importableAccounts?.length ?? 0;
    const numSelectedAccounts = selectedLedgerAccounts.size;
    const isUnlockButtonDisabled = numSelectedAccounts === 0;
    const [, setAccountsFormValues] = useAccountsFormContext();

    const isLoading = areLedgerAccountsLoading || isLoadingMainPublicKey;

    let importLedgerAccountsBody: JSX.Element | null = null;
    if (isLoading) {
        importLedgerAccountsBody = <LedgerViewLoading />;
    } else if (!encounteredDerviceAccountsError) {
        importLedgerAccountsBody = (
            <div className="max-h-[530px] w-full overflow-auto">
                <AccountList
                    accounts={accounts}
                    selectedAccounts={selectedLedgerAccounts}
                    onAccountClick={onAccountClick}
                    selectAll={selectAllAccounts}
                />
            </div>
        );
    }

    function selectAllAccounts() {
        const areAllImportableAccountsSelected = numSelectedAccounts === numImportableAccounts;
        if (importableAccounts && !areAllImportableAccountsSelected) {
            setSelectedLedgerAccounts(new Set(importableAccounts.map((acc) => acc.address)));
        } else if (areAllImportableAccountsSelected) {
            setSelectedLedgerAccounts(new Set());
        }
    }

    function handleNextClick() {
        if (!accounts || !mainPublicKey) {
            return;
        }
        setAccountsFormValues({
            type: AccountsFormType.ImportLedger,
            mainPublicKey: mainPublicKey,
            accounts:
                accounts
                    ?.filter((acc) => selectedLedgerAccounts.has(acc.address))
                    .map(({ address, derivationPath, publicKey }) => ({
                        address,
                        derivationPath,
                        publicKey: publicKey!,
                    })) ?? [],
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
                <div className="flex flex-1 items-end gap-xs">
                    <Button
                        type={ButtonType.Secondary}
                        disabled={isLoading}
                        text="Load More"
                        onClick={() => loadMore()}
                        fullWidth
                    />
                    <Button
                        type={ButtonType.Primary}
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
            <LoadingIndicator />
            <span className="text-title-lg text-iota-neutral-10 dark:text-iota-neutral-92">
                Looking for Accounts...
            </span>
        </div>
    );
}
