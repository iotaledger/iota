// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import {
    ConnectButton,
    useCurrentAccount,
    useSignAndExecuteTransactionBlock,
} from '@iota/dapp-kit';
import { TransactionBlock, getPureSerializationType } from '@iota/iota-sdk/transactions';
import { Button } from '@iota/ui';
import { useMutation } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useWatch } from 'react-hook-form';
import { z } from 'zod';

import { DisclosureBox, Input } from '~/components/ui';
import { useFunctionParamsDetails, useFunctionTypeArguments } from '~/hooks';
import { FunctionExecutionResult } from './FunctionExecutionResult';

import type { IotaMoveNormalizedFunction } from '@iota/iota-sdk/client';
import type { TypeOf } from 'zod';

const argsSchema = z.object({
    params: z.optional(z.array(z.string().trim().min(1))),
    types: z.optional(z.array(z.string().trim().min(1))),
});

type ModuleFunctionProps = {
    packageId: string;
    moduleName: string;
    functionName: string;
    functionDetails: IotaMoveNormalizedFunction;
    defaultOpen?: boolean;
};

export function ModuleFunction({
    defaultOpen,
    packageId,
    moduleName,
    functionName,
    functionDetails,
}: ModuleFunctionProps): JSX.Element {
    const currentAccount = useCurrentAccount();
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();
    const { handleSubmit, formState, register, control } = useZodForm({
        schema: argsSchema,
    });
    const { isValidating, isValid, isSubmitting } = formState;

    const typeArguments = useFunctionTypeArguments(functionDetails.typeParameters);
    const formTypeInputs = useWatch({ control, name: 'types' });
    const resolvedTypeArguments = useMemo(
        () => typeArguments.map((aType, index) => formTypeInputs?.[index] || aType),
        [typeArguments, formTypeInputs],
    );
    const paramsDetails = useFunctionParamsDetails({
        params: functionDetails.parameters,
        functionTypeArgNames: resolvedTypeArguments,
    });

    const execute = useMutation({
        mutationFn: async ({ params, types }: TypeOf<typeof argsSchema>) => {
            const tx = new TransactionBlock();
            tx.moveCall({
                target: `${packageId}::${moduleName}::${functionName}`,
                typeArguments: types ?? [],
                arguments:
                    params?.map((param, i) =>
                        getPureSerializationType(functionDetails.parameters[i], param)
                            ? tx.pure(param)
                            : tx.object(param),
                    ) ?? [],
            });
            const result = await signAndExecuteTransactionBlock({
                transactionBlock: tx,
                options: {
                    showEffects: true,
                    showEvents: true,
                    showInput: true,
                },
            });
            if (result.effects?.status.status === 'failure') {
                throw new Error(result.effects.status.error || 'Transaction failed');
            }
            return result;
        },
    });

    const isExecuteDisabled = isValidating || !isValid || isSubmitting || !currentAccount;

    return (
        <DisclosureBox defaultOpen={defaultOpen} title={functionName}>
            <form
                onSubmit={handleSubmit((formData) =>
                    execute.mutateAsync(formData).catch(() => {
                        /* ignore tx execution errors */
                    }),
                )}
                autoComplete="off"
                className="flex flex-col flex-nowrap items-stretch gap-4"
            >
                {typeArguments.map((aTypeArgument, index) => (
                    <Input
                        key={index}
                        label={`Type${index}`}
                        {...register(`types.${index}` as const)}
                        placeholder={aTypeArgument}
                    />
                ))}
                {paramsDetails.map(({ paramTypeText }, index) => (
                    <Input
                        key={index}
                        label={`Arg${index}`}
                        {...register(`params.${index}` as const)}
                        placeholder={paramTypeText}
                        disabled={isSubmitting}
                    />
                ))}
                <div className="flex items-stretch justify-end gap-1.5">
                    <Button
                        variant="primary"
                        type="submit"
                        disabled={isExecuteDisabled}
                        loading={execute.isPending}
                    >
                        Execute
                    </Button>
                    <ConnectButton />
                </div>
                {execute.error || execute.data ? (
                    <FunctionExecutionResult
                        error={execute.error ? (execute.error as Error).message || 'Error' : false}
                        result={execute.data || null}
                        onClear={() => {
                            execute.reset();
                        }}
                    />
                ) : null}
            </form>
        </DisclosureBox>
    );
}
