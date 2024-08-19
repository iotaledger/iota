// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, type ButtonProps } from '_app/shared/ButtonUI';
import { useAppSelector } from '_hooks';
import { getCustomNetwork } from '_src/shared/api-env';
import { getNetwork } from '@iota/iota-sdk/client';
import { FaucetRateLimitError } from '@iota/iota-sdk/faucet';
import { toast } from 'react-hot-toast';

import FaucetMessageInfo from './FaucetMessageInfo';
import { useFaucetMutation } from './useFaucetMutation';
import { useFaucetRateLimiter } from './useFaucetRateLimiter';

export interface FaucetRequestButtonProps {
    variant?: ButtonProps['variant'];
    size?: ButtonProps['size'];
}

function FaucetRequestButton({ variant = 'primary', size = 'narrow' }: FaucetRequestButtonProps) {
    const network = useAppSelector(({ app }) => app.network);
    const customRpc = useAppSelector(({ app }) => app.customRpc);
    const networkConfig = customRpc ? getCustomNetwork(customRpc) : getNetwork(network);
    const [isRateLimited, rateLimit] = useFaucetRateLimiter();

    const mutation = useFaucetMutation({
        host: networkConfig?.faucet,
        onError: (error) => {
            if (error instanceof FaucetRateLimitError) {
                rateLimit();
            }
        },
    });

    return mutation.enabled ? (
        <Button
            data-testid="faucet-request-button"
            variant={variant}
            size={size}
            disabled={isRateLimited}
            onClick={() => {
                toast.promise(mutation.mutateAsync(), {
                    loading: <FaucetMessageInfo loading />,
                    success: (totalReceived) => <FaucetMessageInfo totalReceived={totalReceived} />,
                    error: (error) => <FaucetMessageInfo error={error.message} />,
                });
            }}
            loading={mutation.isMutating}
            text={`Request ${networkConfig?.name} IOTA Tokens`}
        />
    ) : null;
}

export default FaucetRequestButton;
