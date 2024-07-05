// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FormDataValues } from '../SendCoinPopup';
import { Button } from '@/components';

interface ReviewValuesFormProps {
    formData: FormDataValues;
    senderAddress: string;
    gasBudget: string;
    error: string | undefined;
    isPending: boolean;
    executeTransfer: () => void;
    onBack: () => void;
}

function ReviewValuesFormView({
    formData: { amount, recipientAddress },
    senderAddress,
    gasBudget,
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
                <p>To: {recipientAddress}</p>
                <p>Gas fee: {gasBudget}</p>
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
export default ReviewValuesFormView;
