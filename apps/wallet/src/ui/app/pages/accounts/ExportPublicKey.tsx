// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient, useAccounts } from '_hooks';
import { useMutation } from '@tanstack/react-query';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { VerifyPasswordModal, HideShowDisplayBox, Loading, Overlay } from '_components';
import { fromExportedKeypair } from '_src/shared/utils/keypair';
import { AccountType } from '_src/background/accounts/account';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';

export function ExportPublicKeyPage() {
    const { accountID } = useParams();
    const { data: allAccounts, isPending } = useAccounts();
    const account = allAccounts?.find(({ id }) => accountID === id) || null;
    const backgroundClient = useBackgroundClient();
    const exportMutation = useMutation({
        mutationKey: ['export-account', accountID],
        mutationFn: async (password: string) => {
            if (!account) {
                return null;
            }

            if (account.type === AccountType.LedgerDerived) {
                if (!account.publicKey) {
                    throw new Error('Public key not available for this Ledger account');
                }
                const publicKey = new Ed25519PublicKey(account.publicKey);
                return publicKey.toIotaPublicKey();
            }

            const { keyPair } = await backgroundClient.exportAccountKeyPair({
                password,
                accountID: account.id,
            });
            return fromExportedKeypair(keyPair).getPublicKey().toIotaPublicKey();
        },
        gcTime: 0,
    });
    const navigate = useNavigate();

    if (!account && !isPending) {
        return <Navigate to="/accounts/manage" replace />;
    }
    return (
        <Overlay title="Export Public Key" closeOverlay={() => navigate(-1)} showModal>
            <Loading loading={isPending}>
                {exportMutation.data ? (
                    <div className="flex flex-col gap-md">
                        <HideShowDisplayBox
                            value={exportMutation.data}
                            copiedMessage="Public Key copied"
                        />
                    </div>
                ) : (
                    <VerifyPasswordModal
                        open
                        onVerify={async (password) => {
                            await exportMutation.mutateAsync(password);
                        }}
                        onClose={() => navigate(-1)}
                    />
                )}
            </Loading>
        </Overlay>
    );
}
