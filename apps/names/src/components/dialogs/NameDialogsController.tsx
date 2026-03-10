// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isSubname } from '@iota/iota-names-sdk';
import { Fragment } from 'react';

import { RegistrationNft } from '@/lib/interfaces';

import { DeleteNameDialog } from '.';
import { ConnectToAddressDialog } from './ConnectToAddressDialog';
import { CreateSubnameDialog } from './CreateSubnameDialog';
import { EditMetadataDialog } from './EditMetadata';
import { NameDialogId } from './enums';
import { GeneralInfoDialog } from './GeneralInfoDialog';
import { PersonalizeAvatarDialog } from './PersonalizeAvatarDialog';
import { RenewNameDialog } from './RenewNameDialog';
import { RenewSubnameDialog } from './RenewSubameDialog';
import { SetPermissionsDialog } from './SetPermissionsDialog';

interface NameDialogsControllerProps {
    nft: RegistrationNft;
    openDialogId: NameDialogId | null;
    onClose: () => void;
}

export function NameDialogsController({ nft, openDialogId, onClose }: NameDialogsControllerProps) {
    const isNameSubname = isSubname(nft.name);
    return (
        <Fragment>
            {openDialogId === NameDialogId.Delete ? (
                <DeleteNameDialog nft={nft} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.SetPermissions ? (
                <SetPermissionsDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.CreateSubname ? (
                <CreateSubnameDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.PersonalizeAvatar ? (
                <PersonalizeAvatarDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.RenewName && !isNameSubname ? (
                <RenewNameDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.RenewName && isNameSubname ? (
                <RenewSubnameDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.GeneralInfo ? (
                <GeneralInfoDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.ConnectToAddress ? (
                <ConnectToAddressDialog name={nft.name} setOpen={onClose} />
            ) : null}

            {openDialogId === NameDialogId.EditMetadata ? (
                <EditMetadataDialog name={nft.name} setOpen={onClose} />
            ) : null}
        </Fragment>
    );
}
