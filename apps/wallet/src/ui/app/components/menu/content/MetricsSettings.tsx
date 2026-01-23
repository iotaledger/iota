// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonHtmlType, ButtonType, Checkbox } from '@iota/apps-ui-kit';
import { Overlay } from '_components';
import { useMetricsEnabled } from '_src/ui/app/hooks/useMetricsEnabled';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

export function MetricsSettings() {
    const [hasAcceptedMetrics, setHasAcceptedMetrics] = useMetricsEnabled();
    const [isToggled, setIsToggled] = useState(hasAcceptedMetrics);
    const navigate = useNavigate();

    function onSave() {
        setHasAcceptedMetrics(isToggled);
        navigate('/');
    }

    return (
        <Overlay showModal title="Metrics" closeOverlay={() => navigate('/tokens')} showBackButton>
            <div className="flex flex-1 flex-col">
                <div className="flex flex-1 flex-col gap-6 p-md [&_label]:cursor-pointer  [&_span]:shrink-0">
                    <Checkbox
                        name="metrics.enabled"
                        label="Participate in metrics to help us make the IOTA Wallet better"
                        onCheckedChange={(e) => setIsToggled(e.target.checked)}
                        isChecked={isToggled}
                    />
                </div>
                <div className="flex flex-col gap-4 pt-xxxs">
                    <div className="flex flex-row justify-stretch gap-2.5">
                        <Button
                            type={ButtonType.Primary}
                            disabled={isToggled === hasAcceptedMetrics}
                            text="Save"
                            fullWidth
                            htmlType={ButtonHtmlType.Submit}
                            onClick={onSave}
                        />
                    </div>
                </div>
            </div>
        </Overlay>
    );
}
