// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Loading, Overlay, AutoLockSelector, zodSchema } from '_components';
import {
    autoLockDataToMinutes,
    parseAutoLock,
    useAutoLockMinutes,
} from '_src/ui/app/hooks/useAutoLockMinutes';
import { useAutoLockMinutesMutation } from '_src/ui/app/hooks/useAutoLockMinutesMutation';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { Form } from '_src/ui/app/shared/forms/Form';
import { useZodForm } from '@iota/core';
import toast from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';

export function AutoLockAccounts() {
    const mainMenuUrl = useNextMenuUrl(true, '/');
    const navigate = useNavigate();
    const autoLock = useAutoLockMinutes();
    const savedAutoLockData = parseAutoLock(autoLock.data || null);
    const form = useZodForm({
        mode: 'all',
        schema: zodSchema,
        values: {
            autoLock: savedAutoLockData,
        },
    });
    const {
        formState: { isSubmitting, isValid, isDirty },
    } = form;
    const setAutoLockMutation = useAutoLockMinutesMutation();
    return (
        <Overlay
            showModal={true}
            title={'Auto-lock Accounts'}
            closeOverlay={() => navigate(mainMenuUrl)}
        >
            <Loading loading={autoLock.isPending}>
                <Form
                    className="flex h-full flex-col pt-5"
                    form={form}
                    onSubmit={async (data) => {
                        await setAutoLockMutation.mutateAsync(
                            { minutes: autoLockDataToMinutes(data.autoLock) },
                            {
                                onSuccess: () => {
                                    toast.success('Saved');
                                },
                                onError: (error) => {
                                    toast.error(
                                        (error as Error)?.message || 'Failed, something went wrong',
                                    );
                                },
                            },
                        );
                    }}
                >
                    <AutoLockSelector disabled={isSubmitting} />
                    <div className="flex-1" />
                    <Button
                        type="submit"
                        variant="primary"
                        size="tall"
                        text="Save"
                        disabled={!isValid || !isDirty}
                        loading={isSubmitting}
                    />
                </Form>
            </Loading>
        </Overlay>
    );
}
