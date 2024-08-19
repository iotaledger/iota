// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, HideShowDisplayBox, VerifyPasswordModal, Loading, Overlay } from '_components';
import { Navigate, useNavigate, useParams } from 'react-router-dom';

import { useAccountSources } from '../../hooks/useAccountSources';
import { useExportSeedMutation } from '../../hooks/useExportSeedMutation';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';

export function ExportSeedPage() {
    const { accountSourceID } = useParams();
    const { data: allAccountSources, isPending } = useAccountSources();
    const navigate = useNavigate();
    const exportMutation = useExportSeedMutation();

    const accountSource = allAccountSources?.find(({ id }) => id === accountSourceID) || null;

    if (!isPending && accountSource?.type !== AccountSourceType.Seed) {
        return <Navigate to="/accounts/manage" />;
    }

    return (
        <Overlay title="Export Seed" closeOverlay={() => navigate(-1)} showModal>
            <Loading loading={isPending}>
                {exportMutation.data ? (
                    <div className="flex min-w-0 flex-col gap-3">
                        <Alert>
                            <div className="break-normal">Do not share your Seed!</div>
                            <div className="break-normal">
                                It provides full control of all accounts derived from it.
                            </div>
                        </Alert>
                        <HideShowDisplayBox
                            value={exportMutation.data}
                            copiedMessage="Seed copied"
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
