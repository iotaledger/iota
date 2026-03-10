// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Info, StarHex, Warning } from '@iota/apps-ui-icons';
import { ButtonUnstyled, Tooltip } from '@iota/apps-ui-kit';
import { useCurrentAccount } from '@iota/dapp-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { Fragment } from 'react';

import { MenuButton } from '@/components/buttons/MenuButton';
import { ContextMenuDropdown } from '@/components/ContextMenu';
import { NameDialogsController } from '@/components/dialogs/NameDialogsController';
import { useNameRecord } from '@/hooks';
import { useGetPublicName } from '@/hooks/useGetPublicName';
import { useNameContextMenu } from '@/hooks/useNameContextMenu';
import { useNameManageDialog } from '@/hooks/useNameMenuOptions';
import { useNamesPurchaseMode } from '@/hooks/useNamesPurchaseMode';
import type { RegistrationNft } from '@/lib/interfaces/registration.interfaces';
import { formatExpirationDate } from '@/lib/utils/format/formatExpirationDate';
import { getNameMenuOptions } from '@/lib/utils/getNameMenuOptions';
import {
    getNamePermissions,
    isGracePeriodExpired,
    isNameRecordCloseToExpiration,
    isNameRecordExpired,
} from '@/lib/utils/names';

import { PanelTileType } from './enums';
import { PanelTile } from './PanelTile';

interface NamePanelTileProps {
    registration: RegistrationNft;
    hasSubnames: boolean;
    onClick: () => void;
    onRenewClick: () => void;
}
export function NamePanelTile({
    registration,
    hasSubnames,
    onClick,
    onRenewClick,
}: NamePanelTileProps) {
    const { isVisible, position, toggleMenu, dropdownRef, triggerRef } = useNameContextMenu();
    const { openDialogId, openDialog, closeDialog } = useNameManageDialog();

    const account = useCurrentAccount();
    const { data: publicName } = useGetPublicName(account?.address ?? '');
    const { data: nameRecordData } = useNameRecord(registration.name);

    const nameRecord =
        nameRecordData?.type === 'unavailable' ? nameRecordData.nameRecord : undefined;

    const linkedAddress = nameRecord?.targetAddress || undefined;

    const isPublicName = publicName === registration.name;
    const isCloseToExpiration = isNameRecordCloseToExpiration(registration);
    const isExpired = isNameRecordExpired(registration);
    const allowTimeExtension = nameRecord
        ? getNamePermissions(nameRecord).allowTimeExtension
        : false;

    const { data: paymentsMode } = useNamesPurchaseMode();
    const menuOptions = getNameMenuOptions(
        registration,
        hasSubnames,
        openDialog,
        nameRecordData,
        paymentsMode?.isPaymentAuthorized,
    );

    const panelType = (() => {
        if (isCloseToExpiration) return PanelTileType.Warning;
        if (isExpired) return PanelTileType.Destructive;
        return PanelTileType.Default;
    })();

    const expirationDate = formatExpirationDate(registration.expirationDate);
    const isNameGracePeriodExpired = nameRecord ? isGracePeriodExpired(nameRecord) : undefined;

    return (
        <>
            <PanelTile
                type={panelType}
                icon={
                    isPublicName ? (
                        <Tooltip text="Public name">
                            <StarHex className="w-4 h-4 text-names-primary-80" />
                        </Tooltip>
                    ) : null
                }
                name={registration.name}
                subtitle={linkedAddress ? formatAddress(linkedAddress) : undefined}
                onClick={onClick}
                menuButton={<MenuButton variant="ghost" onClick={toggleMenu} ref={triggerRef} />}
                footer={
                    isCloseToExpiration || isExpired ? (
                        <PanelFooter
                            isRenewDisabled={!allowTimeExtension || isNameGracePeriodExpired}
                            isCloseToExpiration={isCloseToExpiration}
                            isExpired={isExpired}
                            expirationDate={expirationDate}
                            onRenewClick={onRenewClick}
                        />
                    ) : null
                }
            />

            <ContextMenuDropdown
                visible={isVisible}
                position={position}
                options={menuOptions.map((option) => ({
                    ...option,
                    onClick: () => {
                        option.onClick?.();
                        toggleMenu();
                    },
                }))}
                dropdownRef={dropdownRef}
            />

            <NameDialogsController
                nft={registration}
                openDialogId={openDialogId}
                onClose={closeDialog}
            />
        </>
    );
}

interface PanelFooterProps {
    isRenewDisabled?: boolean;
    isCloseToExpiration: boolean;
    isExpired: boolean;
    expirationDate: string;
    onRenewClick?: () => void;
}
function PanelFooter({
    isRenewDisabled,
    isCloseToExpiration,
    isExpired,
    expirationDate,
    onRenewClick,
}: PanelFooterProps) {
    const Icon = isCloseToExpiration ? Info : isExpired ? Warning : Fragment;
    return (
        <div className="flex flex-row w-full gap-x-xs text-body-md justify-between px-sm py-xs">
            <p
                className={clsx(
                    'flex flex-row items-center gap-x-xs [&>svg]:w-4 [&>svg]:h-4',
                    isCloseToExpiration && 'text-names-warning-90',
                    isExpired && 'text-names-error-90',
                )}
            >
                <Icon />
                {isCloseToExpiration ? 'Expires' : 'Expired'} {'\u2022'} {expirationDate}
            </p>

            <ButtonUnstyled
                className="px-xs leading-5 rounded-md enabled:hover:opacity-80 transition-opacity disabled:opacity-60 disabled:cursor-not-allowed"
                onClick={onRenewClick}
                disabled={isRenewDisabled}
            >
                Renew &rarr;
            </ButtonUnstyled>
        </div>
    );
}
