// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient, useAccounts } from '_hooks';
import { useMutation } from '@tanstack/react-query';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { VerifyPasswordModal, HideShowDisplayBox, Loading, Overlay } from '_components';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { AccountType } from '_src/background/accounts/account';

export function ExportAccountPage() {
    const { accountID } = useParams();
    const { data: allAccounts, isPending } = useAccounts();
    const account = allAccounts?.find(({ id }) => accountID === id) || null;
    const isLedgerAccount = account?.type === AccountType.LedgerDerived;
    const isKeystoneAccount = account?.type === AccountType.KeystoneDerived;
    const backgroundClient = useBackgroundClient();
    const exportMutation = useMutation({
        mutationKey: ['export-account', accountID],
        mutationFn: async (password: string) => {
            if (!account || isLedgerAccount) {
                return null;
            }
            return (
                await backgroundClient.exportAccountKeyPair({
                    password,
                    accountID: account.id,
                })
            ).keyPair;
        },
        gcTime: 0,
    });
    const navigate = useNavigate();
    if (!account && !isPending) {
        return <Navigate to="/accounts/manage" replace />;
    }

    const publicKey = account?.publicKey ? new Ed25519PublicKey(account.publicKey) : null;
    return (
        <Overlay title="Export Account Keys" closeOverlay={() => navigate(-1)} showModal>
            <Loading loading={isPending}>
                <div className="max-h-[70vh] overflow-y-auto">
                    <div className="flex flex-col gap-md">
                        <div className="flex flex-col gap-xs">
                            <div className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                Public Key With Flag
                            </div>
                            <HideShowDisplayBox
                                value={publicKey ? publicKey?.toIotaPublicKey() : ''}
                                copiedMessage="Public Key copied"
                                isContentVisible={true}
                            />
                        </div>

                        {!isLedgerAccount && !isKeystoneAccount && (
                            <>
                                {exportMutation.data ? (
                                    <div className="flex flex-col gap-xs">
                                        <InfoBox
                                            icon={<Warning />}
                                            type={InfoBoxType.Warning}
                                            title="Do not share your private key"
                                            supportingText="Your account derived from it can be fully controlled."
                                            style={InfoBoxStyle.Default}
                                        />
                                        <div className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                            Private Key
                                        </div>
                                        <HideShowDisplayBox
                                            value={exportMutation.data}
                                            copiedMessage="Private Key copied"
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
                            </>
                        )}
                    </div>
                </div>
            </Loading>
        </Overlay>
    );
}
