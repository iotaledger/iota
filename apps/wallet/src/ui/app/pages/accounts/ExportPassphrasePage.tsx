// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { HideShowDisplayBox, VerifyPasswordModal, Loading, Overlay } from '_components';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { useAccountSources } from '../../hooks/useAccountSources';
import { useExportPassphraseMutation } from '../../hooks/useExportPassphraseMutation';
import { AccountSourceType } from '_src/background/account-sources/accountSource';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Info } from '@iota/ui-icons';

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
                    <div className="flex flex-col gap-md">
                        <InfoBox
                            icon={<Info />}
                            type={InfoBoxType.Default}
                            title="Do not share your mnemonic"
                            supportingText="All accounts derived from it can be controlled fully."
                            style={InfoBoxStyle.Default}
                        />
                        <HideShowDisplayBox
                            value={exportMutation.data.join(' ')}
                            copiedMessage="Mnemonic copied"
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
