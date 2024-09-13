// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from '_src/ui/app/hooks/useBackgroundClient';
import { useMutation } from '@tanstack/react-query';
import { Navigate, useNavigate, useParams } from 'react-router-dom';

import { VerifyPasswordModal, HideShowDisplayBox, Loading, Overlay } from '_components';
import { useAccounts } from '../../hooks/useAccounts';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Info } from '@iota/ui-icons';

export function ExportAccountPage() {
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
            return (
                await backgroundClient.exportAccountKeyPair({
                    password,
                    accountID: account.id,
                })
            ).keyPair;
        },
    });
    const navigate = useNavigate();
    if (!account && !isPending) {
        return <Navigate to="/accounts/manage" replace />;
    }
    return (
        <Overlay title="Export Private Key" closeOverlay={() => navigate(-1)} showModal>
            <Loading loading={isPending}>
                {exportMutation.data ? (
                    <div className="flex flex-col gap-md">
                        <InfoBox
                            icon={<Info />}
                            type={InfoBoxType.Default}
                            title="Do not share your private key"
                            supportingText="Your account derived from it can be controlled fully."
                            style={InfoBoxStyle.Default}
                        />
                        <HideShowDisplayBox
                            value={exportMutation.data}
                            copiedMessage="Mnemonic copied"
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
