// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useSearchParams } from 'react-router-dom';
import { Form } from '../../shared/forms/Form';

import {
    AccountsFormType,
    PageTemplate,
    useAccountsFormContext,
    useBootstrapSourceFlow,
} from '_components';
import { Input, Button, ButtonHtmlType, ButtonType, InputType } from '@iota/apps-ui-kit';
import { Theme, useTheme, useZodForm } from '@iota/core';
import PasskeyAuthenticationRequired from '_assets/images/passkey_authentication_required.png';
import PasskeyAuthenticationRequiredDarkmode from '_assets/images/passkey_authentication_required_darkmode.png';
import { z } from 'zod';
import { useBackgroundClient } from '../../hooks';

export function ImportPasskeyPage() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const accountID = searchParams.get('accountID') || '';
    useBootstrapSourceFlow();

    return (
        <PageTemplate
            title="Import Passkey Account"
            isTitleCentered
            showBackButton
            onBack={() => navigate('/accounts/import-existing')}
        >
            {accountID ? (
                <NicknameSetContent accountID={accountID} />
            ) : (
                <AuthenticationRequiredContent />
            )}
        </PageTemplate>
    );
}

function AuthenticationRequiredContent() {
    const { theme } = useTheme();
    const [, setAccountsFormValues] = useAccountsFormContext();
    const navigate = useNavigate();

    function handleOnClick() {
        setAccountsFormValues({
            type: AccountsFormType.ImportPasskey,
        });

        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportPasskey,
            }).toString()}`,
        );
    }

    return (
        <div className="flex h-full flex-col">
            <div className="flex w-full flex-1 flex-col items-center justify-start gap-y-lg pb-md--rs">
                <div className="flex flex-col items-center gap-lg">
                    <img
                        src={
                            theme === Theme.Dark
                                ? PasskeyAuthenticationRequiredDarkmode
                                : PasskeyAuthenticationRequired
                        }
                        alt="Passkey Authentication Required"
                        height={210}
                        width="auto"
                        className="aspect-[4/3] h-[210px] w-auto object-cover"
                    />

                    <div className="flex flex-col items-center justify-center gap-xs text-center">
                        <h4 className="text-title-lg text-iota-neutral-10 dark:text-iota-neutral-92">
                            Authentication Required
                        </h4>
                        <p className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                            In the next steps, you’ll be asked to confirm your identity twice using
                            your device PIN or biometrics. This is a normal part of importing a
                            passkey.
                        </p>
                    </div>
                </div>
            </div>
            <div className="flex flex-col gap-4 pt-xs">
                <div className="flex flex-row justify-stretch gap-2.5">
                    <Button
                        type={ButtonType.Primary}
                        text="Continue"
                        fullWidth
                        onClick={handleOnClick}
                    />
                </div>
            </div>
        </div>
    );
}

const formSchema = z.object({
    username: z.string().max(50, 'Username is too long').optional(),
});

type FormValues = z.infer<typeof formSchema>;

function NicknameSetContent({ accountID }: { accountID: string }) {
    const navigate = useNavigate();
    const backgroundClient = useBackgroundClient();

    const form = useZodForm({
        mode: 'onChange',
        schema: formSchema,
        defaultValues: {
            username: '',
        },
    });

    async function handleSubmit(values: FormValues) {
        if (values.username) {
            await backgroundClient.setAccountNickname({
                id: accountID,
                nickname: values.username,
            });
        }
        navigate('/tokens', { replace: true });
    }

    return (
        <Form className="flex h-full w-full flex-col" form={form} onSubmit={handleSubmit}>
            <div className="flex w-full flex-1 flex-col items-center justify-start gap-y-lg pb-md--rs">
                <Input
                    autoFocus
                    type={InputType.Text}
                    label="Account Nickname"
                    placeholder="Give your account a name"
                    errorMessage={form.formState.errors.username?.message}
                    {...form.register('username', { shouldUnregister: true })}
                    name="username"
                    data-testid="username-input"
                />
            </div>
            <div className="flex flex-col gap-4 pt-xs">
                <div className="flex flex-row justify-stretch gap-2.5">
                    <Button
                        type={ButtonType.Primary}
                        text="Continue"
                        fullWidth
                        htmlType={ButtonHtmlType.Submit}
                    />
                </div>
            </div>
        </Form>
    );
}
