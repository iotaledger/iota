// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppDispatch, useAppSelector } from '_hooks';
import { changeActiveNetwork } from '_redux/slices/app';
import { ampli } from '_src/shared/analytics/ampli';
import { isValidUrl } from '_src/shared/utils';
import { Network } from '@iota/iota-sdk/client';
import { Form, Formik } from 'formik';
import { useCallback } from 'react';
import { toast } from '@iota/core';
import * as Yup from 'yup';
import { InputWithAction } from './InputWithAction';

const MIN_CHAR = 5;

const validation = Yup.object({
    rpcInput: Yup.string()
        .required()
        .label('Custom RPC URL')
        .min(MIN_CHAR)
        .test('validate-url', 'Not a valid URL', (value) => isValidUrl(value || null)),
});

export function CustomRPCInput() {
    const placeholder = 'http://localhost:3000/';

    const customRpc = useAppSelector(({ app }) => app.customRpc || '');

    const dispatch = useAppDispatch();

    const changeNetwork = useCallback(
        async ({ rpcInput }: { rpcInput: string }) => {
            try {
                await dispatch(
                    changeActiveNetwork({
                        network: {
                            network: Network.Custom,
                            customRpcUrl: rpcInput,
                        },
                        store: true,
                    }),
                ).unwrap();
                ampli.switchedNetwork({
                    toNetwork: rpcInput,
                });
            } catch (e) {
                toast.error((e as Error).message);
            }
        },
        [dispatch],
    );

    return (
        <Formik
            initialValues={{ rpcInput: customRpc }}
            validationSchema={validation}
            onSubmit={changeNetwork}
            enableReinitialize={true}
        >
            <Form>
                <InputWithAction name="rpcInput" min={MIN_CHAR} placeholder={placeholder} />
            </Form>
        </Formik>
    );
}
