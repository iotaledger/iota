// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useAutoLockMinutesMutation,
    useResetPasswordMutation,
    autoLockDataToMinutes,
} from '_hooks';
import { toast } from '@iota/core';
import { Navigate, useNavigate } from 'react-router-dom';
import { ProtectAccountForm, type ProtectAccountFormValues } from '_components';
import { useForgotPasswordContext } from './ForgotPasswordPage';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';

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
