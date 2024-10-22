// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from '_src/ui/app/hooks/useBackgroundClient';
import { Text } from '_src/ui/app/shared/text';
import classNames from 'clsx';
import { Form, Formik } from 'formik';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import { object, string as YupString } from 'yup';
import { ArrowLeft, ArrowRight } from '@iota/ui-icons';
import {
    Header,
    InputType,
    LoadingIndicator,
    ButtonSize,
    ButtonType,
    ButtonHtmlType,
    Button,
} from '@iota/apps-ui-kit';
import { PasswordInputField } from '../shared/input/password';

const validation = object({
    password: YupString().ensure().required().label('Password'),
});

export interface PasswordExportDialogProps {
    title: string;
    continueLabel?: string;
    showArrowIcon?: boolean;
    onPasswordVerified: (password: string) => Promise<void> | void;
    onBackClicked?: () => void;
    showBackButton?: boolean;
    spacing?: boolean;
    background?: boolean;
}

/** @deprecated - use UnlockAccountModal instead **/
export function PasswordInputDialog({
    title,
    continueLabel = 'Continue',
    showArrowIcon = false,
    spacing = false,
    background = false,
    onPasswordVerified,
    onBackClicked,
    showBackButton = false,
}: PasswordExportDialogProps) {
    const navigate = useNavigate();
    const backgroundService = useBackgroundClient();
    return (
        <Formik
            initialValues={{ password: '' }}
            onSubmit={async ({ password }, { setFieldError }) => {
                try {
                    await backgroundService.verifyPassword({ password });
                    try {
                        await onPasswordVerified(password);
                    } catch (e) {
                        toast.error((e as Error).message || 'Wrong password');
                    }
                } catch (e) {
                    setFieldError('password', (e as Error).message || 'Wrong password');
                }
            }}
            validationSchema={validation}
            validateOnMount
        >
            {({ isSubmitting, isValid, errors }) => (
                <Form
                    className={classNames('flex flex-1 flex-col flex-nowrap items-center gap-7.5', {
                        'bg-white': background,
                        'px-5 pt-10': spacing,
                    })}
                >
                    <Header title={title} titleCentered />
                    <div className="flex-1 self-stretch">
                        <PasswordInputField
                            name="password"
                            type={InputType.Password}
                            label="Enter your wallet password to continue"
                            errorMessage={errors.password}
                        />
                        <div className="mt-4 text-center">
                            <Text variant="pBodySmall" color="steel-dark" weight="normal">
                                This is the password you currently use to lock and unlock your IOTA
                                wallet.
                            </Text>
                        </div>
                    </div>
                    <div className="flex flex-nowrap gap-3.75 self-stretch">
                        {showBackButton ? (
                            <Button
                                fullWidth
                                size={ButtonSize.Small}
                                type={ButtonType.Outlined}
                                text="Back"
                                icon={<ArrowLeft className="h-4 w-4" />}
                                onClick={() => {
                                    if (typeof onBackClicked === 'function') {
                                        onBackClicked();
                                    } else {
                                        navigate(-1);
                                    }
                                }}
                                disabled={isSubmitting}
                            />
                        ) : null}
                        <Button
                            fullWidth
                            htmlType={ButtonHtmlType.Submit}
                            type={ButtonType.Primary}
                            size={ButtonSize.Small}
                            text={continueLabel}
                            icon={
                                isSubmitting ? (
                                    <LoadingIndicator />
                                ) : (
                                    <ArrowRight className="h-4 w-4" />
                                )
                            }
                            iconAfterText
                            disabled={isSubmitting || !isValid}
                        />
                    </div>
                </Form>
            )}
        </Formik>
    );
}
