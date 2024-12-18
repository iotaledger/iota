// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
	getLedgerConnectionErrorMessage,
	getIotaApplicationErrorMessage,
} from '_src/ui/app/helpers/errorMessages';
import { Link } from '_src/ui/app/shared/Link';
import { Text } from '_src/ui/app/shared/text';
import { Check12, X12 } from '@iota/icons';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { useState } from 'react';
import toast from 'react-hot-toast';

import { useIotaLedgerClient } from '../../ledger/IotaLedgerClientProvider';
import LoadingIndicator from '../../loading/LoadingIndicator';

export type VerifyLedgerConnectionLinkProps = {
	accountAddress: string;
	derivationPath: string;
};

enum VerificationStatus {
	UNKNOWN = 'UNKNOWN',
	VERIFIED = 'VERIFIED',
	NOT_VERIFIED = 'NOT_VERIFIED',
}

const resetVerificationStatusDelay = 5000;
const loadingStateDelay = 200;

export function VerifyLedgerConnectionStatus({
	accountAddress,
	derivationPath,
}: VerifyLedgerConnectionLinkProps) {
	const { connectToLedger } = useIotaLedgerClient();
	const [isPending, setLoading] = useState(false);
	const [verificationStatus, setVerificationStatus] = useState(VerificationStatus.UNKNOWN);

	switch (verificationStatus) {
		case VerificationStatus.UNKNOWN:
			if (isPending) {
				return (
					<div className="flex gap-1 text-hero-dark">
						<LoadingIndicator color="inherit" />
						<Text variant="bodySmall">Please confirm on your Ledger...</Text>
					</div>
				);
			}

			return (
				<Link
					text="Verify Ledger connection"
					onClick={async () => {
						const loadingTimeoutId = setTimeout(() => {
							setLoading(true);
						}, loadingStateDelay);

						try {
							const iotaLedgerClient = await connectToLedger();
							const publicKeyResult = await iotaLedgerClient.getPublicKey(derivationPath, true);
							const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
							const iotaAddress = publicKey.toIotaAddress();

							setVerificationStatus(
								accountAddress === iotaAddress
									? VerificationStatus.VERIFIED
									: VerificationStatus.NOT_VERIFIED,
							);
						} catch (error) {
							const errorMessage =
								getLedgerConnectionErrorMessage(error) ||
								getIotaApplicationErrorMessage(error) ||
								'Something went wrong';
							toast.error(errorMessage);

							setVerificationStatus(VerificationStatus.NOT_VERIFIED);
						} finally {
							clearTimeout(loadingTimeoutId);
							setLoading(false);

							window.setTimeout(() => {
								setVerificationStatus(VerificationStatus.UNKNOWN);
							}, resetVerificationStatusDelay);
						}
					}}
					color="heroDark"
					weight="medium"
				/>
			);
		case VerificationStatus.NOT_VERIFIED:
			return (
				<div className="flex items-center gap-1">
					<X12 className="text-issue-dark" />
					<Text variant="bodySmall" color="issue-dark">
						Ledger is not connected
					</Text>
				</div>
			);
		case VerificationStatus.VERIFIED:
			return (
				<div className="flex items-center gap-1">
					<Check12 className="text-success-dark" />
					<Text variant="bodySmall" color="success-dark">
						Ledger is connected
					</Text>
				</div>
			);
		default:
			throw new Error(`Encountered unknown verification status ${verificationStatus}`);
	}
}
