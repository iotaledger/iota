// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Alert from '_components/alert';
import { HideShowDisplayBox } from '_components/HideShowDisplayBox';
import { Navigate, useNavigate, useParams } from 'react-router-dom';

import { VerifyPasswordModal } from '_components/accounts/VerifyPasswordModal';
import Loading from '_components/loading';
import Overlay from '_components/overlay';
import { useAccountSources } from '_app/hooks/useAccountSources';
import { useExportPassphraseMutation } from '_app/hooks/useExportPassphraseMutation';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';

export function ExportPassphrasePage() {
    const { accountSourceID } = useParams();
    const { data: allAccountSources, isPending } = useAccountSources();
    const accountSource = allAccountSources?.find(({ id }) => id === accountSourceID) || null;
    const navigate = useNavigate();
    const exportMutation = useExportPassphraseMutation();
    if (!isPending && accountSource?.type !== AccountSourceType.Mnemonic) {
        return <Navigate to="/accounts/manage" />;
    }
    return (
        <Overlay title="Export Passphrase" closeOverlay={() => navigate(-1)} showModal>
            <Loading loading={isPending}>
                {exportMutation.data ? (
                    <div className="flex min-w-0 flex-col gap-3">
                        <Alert>
                            <div className="break-normal">Do not share your Passphrase!</div>
                            <div className="break-normal">
                                It provides full control of all accounts derived from it.
                            </div>
                        </Alert>
                        <HideShowDisplayBox
                            value={exportMutation.data}
                            copiedMessage="Passphrase copied"
                        />
                    </div>
                ) : (
                    <VerifyPasswordModal
                        open
                        onVerify={async (password) => {
                            await exportMutation.mutateAsync({
                                password,
                                accountSourceID: accountSource!.id,
                            });
                        }}
                        onClose={() => navigate(-1)}
                    />
                )}
            </Loading>
        </Overlay>
    );
}
