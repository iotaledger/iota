// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from '_hooks';
import classNames from 'clsx';
import { Form, Formik } from 'formik';
import { toast } from '@iota/core';
import { useNavigate } from 'react-router-dom';
import { object, string as YupString } from 'yup';
import { ArrowLeft, ArrowRight, Loader } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    ButtonSize,
    Header,
    InputType,
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
                        'bg-neutral-100 dark:bg-neutral-6': background,
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
                            <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                                This is the password you currently use to lock and unlock your IOTA
                                wallet.
                            </span>
                        </div>
                    </div>
                    <div className="gap-3.75 flex flex-nowrap self-stretch">
                        {showBackButton ? (
                            <Button
                                size={ButtonSize.Small}
                                text="Back"
                                type={ButtonType.Secondary}
                                icon={<ArrowLeft className="h-4 w-4" />}
                                onClick={() => {
                                    if (typeof onBackClicked === 'function') {
                                        onBackClicked();
                                    } else {
                                        navigate(-1);
                                    }
                                }}
                                disabled={isSubmitting}
                                fullWidth
                            />
                        ) : null}
                        <Button
                            size={ButtonSize.Small}
                            htmlType={ButtonHtmlType.Submit}
                            type={ButtonType.Primary}
                            text={continueLabel}
                            disabled={isSubmitting || !isValid}
                            icon={
                                isSubmitting ? (
                                    <Loader className="h-4 w-4 animate-spin" />
                                ) : (
                                    <ArrowRight className="h-4 w-4" />
                                )
                            }
                            iconAfterText
                            fullWidth
                        />
                    </div>
                </Form>
            )}
        </Formik>
    );
}
