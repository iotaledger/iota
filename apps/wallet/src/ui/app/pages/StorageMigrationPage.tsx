// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMutation } from '@tanstack/react-query';
import { toast } from '@iota/core';
import { VerifyPasswordModal } from '_components/accounts';
import { useBackgroundClient, useStorageMigrationStatus } from '_hooks';
import { CardLayout } from '../shared/card-layout';
import { Toaster } from '../shared/toaster';
import { LoadingIndicator } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';

export function StorageMigrationPage() {
    const { data } = useStorageMigrationStatus();
    const backgroundClient = useBackgroundClient();
    const migrationMutation = useMutation({
        mutationKey: ['do storage migration'],
        mutationFn: ({ password }: { password: string }) =>
            backgroundClient.doStorageMigration({ password }),
        onSuccess: () => {
            toast.success('Storage migration done');
        },
    });
    const navigate = useNavigate();
    if (!data || data === 'ready') {
        return null;
    }
    return (
        <>
            <CardLayout
                title={data === 'inProgress' ? 'Storage migration in progress, please wait' : ''}
                subtitle={data === 'required' ? 'Storage migration is required' : ''}
                icon="iota"
            >
                {data === 'required' && !migrationMutation.isSuccess ? (
                    <VerifyPasswordModal
                        open
                        onVerify={async (password) => {
                            await migrationMutation.mutateAsync({ password });
                        }}
                        onClose={() => navigate(-1)}
                    />
                ) : (
                    <div className="flex flex-1 items-center">
                        <LoadingIndicator />
                    </div>
                )}
            </CardLayout>
            <Toaster />
        </>
    );
}
