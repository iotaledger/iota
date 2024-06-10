// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FormDataValues } from '../SendCoinPopup';
import { Button } from '@/components';

interface ReviewValuesFormProps {
    formData: FormDataValues;
    handleBack: () => void;
}

function ReviewValuesFormView({
    formData: { amount, senderAddress, recipientAddress },
    handleBack,
}: ReviewValuesFormProps): JSX.Element {
    function onSend(): void {
        console.log('Sending coins');
    }

    return (
        <div className="flex flex-col gap-4">
            <h1 className="mb-4 text-center text-xl">Review & Send</h1>
            <div className="flex flex-col gap-4">
                <p>Sending: {amount}</p>
                <p>From: {senderAddress}</p>
                <p>To: {recipientAddress}</p>
            </div>
            <div className="mt-4 flex justify-around">
                <Button onClick={handleBack}>Back</Button>
                <Button onClick={onSend}>Send now</Button>
            </div>
        </div>
    );
}
export default ReviewValuesFormView;
