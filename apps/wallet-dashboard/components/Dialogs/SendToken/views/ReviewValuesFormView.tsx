// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '@/components';
import { FormDataValues } from '../interfaces';

interface ReviewValuesFormProps {
    formData: FormDataValues;
    senderAddress: string;
    error: string | undefined;
    isPending: boolean;
    executeTransfer: () => void;
    onBack: () => void;
}

export function ReviewValuesFormView({
    formData: { amount, to, gasBudgetEst },
    senderAddress,
    error,
    isPending,
    executeTransfer,
    onBack,
}: ReviewValuesFormProps): JSX.Element {
    return (
        <div className="flex flex-col gap-4">
            <h1 className="mb-4 text-center text-xl">Review & Send</h1>
            <div className="flex flex-col gap-4">
                <p>Sending: {amount}</p>
                <p>From: {senderAddress}</p>
                <p>To: {to}</p>
                <p>Gas fee: {gasBudgetEst}</p>
            </div>
            {error ? <span className="text-red-700">{error}</span> : null}
            <div className="mt-4 flex justify-around">
                <Button onClick={onBack}>Back</Button>
                {isPending ? (
                    <Button disabled>Loading...</Button>
                ) : (
                    <Button onClick={executeTransfer}>Send now</Button>
                )}
            </div>
        </div>
    );
}
