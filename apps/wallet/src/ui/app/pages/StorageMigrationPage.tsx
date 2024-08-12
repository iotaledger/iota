// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMutation } from '@tanstack/react-query';
import { toast } from 'react-hot-toast';

import LoadingIndicator from '_components/loading/LoadingIndicator';
import { PasswordInputDialog } from '_components/PasswordInputDialog';
import { useBackgroundClient } from '_app/hooks/useBackgroundClient';
import { useStorageMigrationStatus } from '_app/hooks/useStorageMigrationStatus';
import { CardLayout } from '_app/shared/card-layout';
import { Toaster } from '_app/shared/toaster';

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
                    <PasswordInputDialog
                        onPasswordVerified={async (password) => {
                            await migrationMutation.mutateAsync({ password });
                        }}
                        title="Please insert your wallet password"
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
