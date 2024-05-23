import { useCurrentAccount } from '@mysten/dapp-kit';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';

import { useBalance } from '../../hooks/useBalance';

export const AccountBalance = () => {
	const account = useCurrentAccount();

	const { calculateBalance, getBalanceQuery } = useBalance(SUI_TYPE_ARG, account?.address);

	return (
		<div>
			{getBalanceQuery?.isLoading && <p>Loading...</p>}
			{!getBalanceQuery?.isLoading && <p>Balance: {calculateBalance}</p>}
		</div>
	);
};
