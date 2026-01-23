// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Toggle } from '@iota/apps-ui-kit';
import { Overlay } from '_components';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

export function MetricsSettings() {
    const [isToggled, setIsToggled] = useState(true);
    const navigate = useNavigate();

    return (
        <Overlay showModal title="Metrics" closeOverlay={() => navigate('/tokens')} showBackButton>
            <div className="flex w-full flex-1 flex-col p-md">
                <p className=" text-label-lg text-iota-neutral-60 dark:text-iota-neutral-40">
                    Participate in metrics to help us make the IOTA Wallet better
                </p>
                <Toggle isToggled={isToggled} onChange={(e) => setIsToggled(e)} />
            </div>
        </Overlay>
    );
}
