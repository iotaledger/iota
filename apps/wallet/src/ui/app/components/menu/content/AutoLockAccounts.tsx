// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Loading, Overlay, AutoLockSelector, zodSchema } from '_components';
import {
    autoLockDataToMinutes,
    parseAutoLock,
    useAutoLockMinutes,
    useAutoLockMinutesMutation,
} from '_hooks';
import { Form } from '_src/ui/app/shared/forms/Form';
import { useZodForm, toast } from '@iota/core';
import { useNavigate } from 'react-router-dom';
import { Button, ButtonHtmlType, ButtonType } from '@iota/apps-ui-kit';

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

    async function handleSave(data: { autoLock: ReturnType<typeof parseAutoLock> }) {
        await setAutoLockMutation.mutateAsync(
            { minutes: autoLockDataToMinutes(data.autoLock) },
            {
                onSuccess: () => {
                    toast.success('Saved');
                    navigate(mainMenuUrl);
                },
                onError: (error) => {
                    toast.error((error as Error)?.message || 'Failed, something went wrong');
                },
            },
        );
    }
    return (
        <Overlay
            showModal={true}
            title="Auto Lock Profile"
            closeOverlay={() => navigate(mainMenuUrl)}
        >
            <Loading loading={autoLock.isPending}>
                <Form className="flex h-full flex-col" form={form} onSubmit={handleSave}>
                    <AutoLockSelector disabled={isSubmitting} />
                    <div className="flex-1" />
                    <Button
                        type={ButtonType.Primary}
                        htmlType={ButtonHtmlType.Submit}
                        text="Save"
                        disabled={!isValid || !isDirty}
                    />
                </Form>
            </Loading>
        </Overlay>
    );
}
