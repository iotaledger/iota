// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SupplyIncreaseVestingPortfolio } from '@/lib/interfaces';
import { Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { VestingScheduleBox } from './VestingScheduleBox';

interface VestingScheduleDialogProps {
    setOpen: (bool: boolean) => void;
    open: boolean;
    vestingPortfolio: SupplyIncreaseVestingPortfolio;
}

export function VestingScheduleDialog({
    open,
    setOpen,
    vestingPortfolio,
}: VestingScheduleDialogProps): React.JSX.Element {
    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent
                containerId="overlay-portal-container"
                customWidth="max-w-md sm:max-w-xl md:max-w-5xl w-full"
            >
                <Header title="Rewards Schedule" onClose={() => setOpen(false)} titleCentered />
                <DialogBody>
                    <div className="h-[440px] overflow-y-auto">
                        <div className="grid grid-cols-1 gap-sm sm:grid-cols-2 md:grid-cols-4">
                            {vestingPortfolio?.map((vestingObject, index) => (
                                <VestingScheduleBox
                                    key={index}
                                    amount={vestingObject.amount}
                                    expirationTimestampMs={vestingObject.expirationTimestampMs}
                                />
                            ))}
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
