import { useGetAllCoins } from '@mysten/core/src/hooks/useGetAllCoins';
import { useCurrentAccount } from '@mysten/dapp-kit';

const COIN_TYPE = '0x2::sui::SUI';

export const AllCoins = () => {
	const account = useCurrentAccount();
	const { data } = useGetAllCoins(COIN_TYPE, account?.address);

	return (
		<div>
			Coins:
			{data?.map((coin) => {
				return (
					<div key={coin.coinObjectId}>
						{coin.balance} - {coin.coinObjectId}
					</div>
				);
			})}
		</div>
	);
};
