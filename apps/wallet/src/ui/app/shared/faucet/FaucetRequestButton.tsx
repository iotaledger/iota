// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector } from '_hooks';
import { getCustomNetwork } from '_src/shared/api-env';
import { getNetwork } from '@iota/iota-sdk/client';
import { FaucetRateLimitError } from '@iota/iota-sdk/faucet';
import { toast } from 'react-hot-toast';
import FaucetMessageInfo from './FaucetMessageInfo';
import { useFaucetMutation } from './useFaucetMutation';
import { useFaucetRateLimiter } from './useFaucetRateLimiter';
import { Button, ButtonType } from '@iota/apps-ui-kit';

function FaucetRequestButton(): JSX.Element | null {
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
            type={ButtonType.Secondary}
            disabled={isRateLimited}
            onClick={() => {
                toast.promise(mutation.mutateAsync(), {
                    loading: <FaucetMessageInfo loading />,
                    success: (totalReceived) => <FaucetMessageInfo totalReceived={totalReceived} />,
                    error: (error) => <FaucetMessageInfo error={error.message} />,
                });
            }}
            text={`Request ${networkConfig?.name} Tokens`}
        />
    ) : null;
}

export default FaucetRequestButton;
