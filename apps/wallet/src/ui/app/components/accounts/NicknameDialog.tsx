// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import toast from 'react-hot-toast';
import { z } from 'zod';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { useAccounts, useBackgroundClient } from '_hooks';
import { Form } from '../../shared/forms/Form';

const formSchema = z.object({
    nickname: z.string().trim(),
});

interface NicknameDialogProps {
    accountID: string;
    isOpen: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function NicknameDialog({ isOpen, setOpen, accountID }: NicknameDialogProps) {
    const backgroundClient = useBackgroundClient();
    const { data: accounts } = useAccounts();
    const account = accounts?.find((account) => account.id === accountID);

    const form = useZodForm({
        mode: 'all',
        schema: formSchema,
        defaultValues: {
            nickname: account?.nickname ?? '',
        },
    });
    const {
        register,
        formState: { isSubmitting, isValid },
    } = form;

    const onSubmit = async ({ nickname }: { nickname: string }) => {
        if (account && accountID) {
            try {
                await backgroundClient.setAccountNickname({
                    id: accountID,
                    nickname: nickname || null,
                });
                setOpen(false);
            } catch (e) {
                toast.error((e as Error).message || 'Failed to set nickname');
            }
        }
    };

    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Account Nickname" onClose={() => setOpen(false)} />
                <DialogBody>
                    <Form className="flex h-full flex-col gap-6" form={form} onSubmit={onSubmit}>
                        <Input
                            type={InputType.Text}
                            label="Personalize account with a nickname."
                            {...register('nickname')}
                        />
                        <div className="flex gap-2.5">
                            <Button
                                type={ButtonType.Secondary}
                                text="Cancel"
                                onClick={() => setOpen(false)}
                                fullWidth
                            />
                            <Button
                                htmlType={ButtonHtmlType.Submit}
                                type={ButtonType.Primary}
                                disabled={isSubmitting || !isValid}
                                text="Save"
                                fullWidth
                            />
                        </div>
                    </Form>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
