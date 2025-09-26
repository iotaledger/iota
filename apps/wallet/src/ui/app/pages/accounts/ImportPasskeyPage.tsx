// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import { AccountsFormType, PageTemplate, useAccountsFormContext } from '_components';
import { Button, ButtonHtmlType, ButtonType, Input, InputType, Toggle } from '@iota/apps-ui-kit';
import { useZodForm } from '@iota/core';
import { z } from 'zod';
import { Form } from '../../shared/forms/Form';

const formSchema = z.object({
    username: z.string().min(1, 'Username is required').max(50, 'Username is too long'),
    displayName: z.string().optional(),
    isPlatformAuthenticator: z.boolean().default(true),
    isRestoreAccount: z.boolean().default(false),
});
type ImportPasskeyFormValues = z.infer<typeof formSchema>;

export function ImportPasskeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    const form = useZodForm({
        mode: 'onChange',
        schema: formSchema,
        defaultValues: {
            username: '',
            displayName: '',
            isPlatformAuthenticator: true,
            isRestoreAccount: false,
        },
    });

    const {
        register,
        formState: { isSubmitting, isValid, errors },
    } = form;

    const handleSubmit = async (values: ImportPasskeyFormValues) => {
        setAccountsFormValues({
            type: AccountsFormType.Passkey,
            authenticatorAttachment: values.isPlatformAuthenticator ? 'platform' : 'cross-platform',
            username: values.username,
            displayName: values.displayName || values.username,
            isRestoreAccount: values.isRestoreAccount,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.Passkey,
            }).toString()}`,
        );
    };

    return (
        <PageTemplate title="Create Passkey Account" isTitleCentered showBackButton>
            <Form
                className="flex h-full flex-col justify-between"
                form={form}
                onSubmit={handleSubmit}
            >
                <div className="flex flex-col gap-6">
                    <Input
                        autoFocus
                        type={InputType.Text}
                        label="Username"
                        placeholder="Enter username"
                        errorMessage={errors.username?.message}
                        {...register('username', { shouldUnregister: true })}
                        name="username"
                        data-testid="username-input"
                    />

                    <Input
                        autoFocus
                        type={InputType.Text}
                        label="Display Name (Optional)"
                        placeholder="Enter nickname"
                        errorMessage={errors.displayName?.message}
                        {...register('displayName', { shouldUnregister: true })}
                        name="displayName"
                        data-testid="display-name-input"
                    />

                    <div className="flex flex-col gap-2">
                        <label className="text-label-md text-iota-neutral-30 dark:text-iota-neutral-80">
                            Authenticator Type
                        </label>

                        <div className="flex flex-col gap-3">
                            <Toggle
                                label={
                                    form.watch('isPlatformAuthenticator')
                                        ? 'Platform'
                                        : 'Cross-Platform'
                                }
                                isToggled={form.watch('isPlatformAuthenticator')}
                                onChange={(isToggled) =>
                                    form.setValue('isPlatformAuthenticator', isToggled)
                                }
                                name="isPlatformAuthenticator"
                                testId="platform-authenticator-toggle"
                            />
                        </div>
                        <div className="flex flex-col gap-3">
                            <Toggle
                                label={
                                    form.watch('isRestoreAccount')
                                        ? 'Restore Existing Account'
                                        : 'Create New Account'
                                }
                                isToggled={form.watch('isRestoreAccount')}
                                onChange={(isToggled) =>
                                    form.setValue('isRestoreAccount', isToggled)
                                }
                                name="isRestoreAccount"
                                testId="restore-account-toggle"
                            />
                        </div>
                    </div>
                </div>

                <div className="flex flex-col gap-4 pt-xxxs">
                    <div className="flex flex-row justify-stretch gap-2.5">
                        <Button
                            type={ButtonType.Secondary}
                            text="Cancel"
                            onClick={() => navigate(-1)}
                            fullWidth
                        />
                        <Button
                            type={ButtonType.Primary}
                            disabled={isSubmitting || !isValid}
                            text={form.watch('isRestoreAccount') ? 'Restore' : 'Create'}
                            fullWidth
                            htmlType={ButtonHtmlType.Submit}
                        />
                    </div>
                </div>
            </Form>
        </PageTemplate>
    );
}
