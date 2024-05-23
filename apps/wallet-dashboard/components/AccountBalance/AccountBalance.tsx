import { useCurrentAccount } from '@mysten/dapp-kit';

import { useBalance } from '../../hooks/useBalance';

const COIN_TYPE = '0x2::sui::SUI';

export const AccountBalance = () => {
	const account = useCurrentAccount();

	const { calculateBalance, getBalanceQuery } = useBalance(COIN_TYPE, account?.address);

	return (
		<div>
			{getBalanceQuery?.isLoading && <p>Loading...</p>}
			{!getBalanceQuery?.isLoading && <p>Balance: {calculateBalance}</p>}
		</div>
	);
};
