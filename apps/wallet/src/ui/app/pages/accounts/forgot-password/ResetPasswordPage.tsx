// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAutoLockMinutesMutation } from '_src/ui/app/hooks/useAutoLockMinutesMutation';
import { useResetPasswordMutation } from '_src/ui/app/hooks/useResetPasswordMutation';
import { toast } from 'react-hot-toast';
import { Navigate, useNavigate } from 'react-router-dom';
import { ProtectAccountForm } from '_components';
import { autoLockDataToMinutes } from '../../../hooks/useAutoLockMinutes';
import { useForgotPasswordContext } from './ForgotPasswordPage';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';
import type { ProtectAccountFormValues } from '_src/ui/app/components/accounts/ProtectAccountForm';

export function ResetPasswordPage() {
    const { value, clear } = useForgotPasswordContext();
    const autoLockMutation = useAutoLockMinutesMutation();
    const resetPasswordMutation = useResetPasswordMutation();
    const navigate = useNavigate();
    if (!value.length && !resetPasswordMutation.isSuccess) {
        return <Navigate to="/accounts/forgot-password" replace />;
    }

    async function handleOnSubmit({ password, autoLock }: ProtectAccountFormValues) {
        try {
            await autoLockMutation.mutateAsync({
                minutes: autoLockDataToMinutes(autoLock),
            });
            await resetPasswordMutation.mutateAsync({
                password: password.input,
                recoveryData: value,
            });
            clear();
            toast.success('Password reset');
            navigate('/');
        } catch (e) {
            toast.error((e as Error)?.message || 'Something went wrong');
        }
    }
    return (
        <PageTemplate title="Reset password" isTitleCentered showBackButton>
            <div className="flex h-full flex-col items-center">
                <div className="mt-6 w-full grow">
                    <ProtectAccountForm
                        cancelButtonText="Back"
                        submitButtonText="Reset Password"
                        onSubmit={handleOnSubmit}
                        hideToS
                    />
                </div>
            </div>
        </PageTemplate>
    );
}
