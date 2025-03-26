// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { HideShowDisplayBox, VerifyPasswordModal, Loading, Overlay } from '_components';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { useAccountSources, useExportSeedMutation } from '_hooks';
import { AccountSourceType } from '_src/background/account-sources/accountSource';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

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
                    <div className="flex flex-col gap-md">
                        <InfoBox
                            icon={<Warning />}
                            type={InfoBoxType.Warning}
                            title="Do not share your seed"
                            supportingText="All accounts derived from it can be controlled fully."
                            style={InfoBoxStyle.Default}
                        />
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
