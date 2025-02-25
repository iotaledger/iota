# Step 1: Call airdrop to get 1 coin
iota client call --package 0x7641ee891b657e349e7c34bcb0ad78bbd8ac9e41c7bcd627f822625e7b725f67 --module airdrop --function airdrop --args 0x5c154e5199c1ebbbc839c7bb19d0f44f6e82b8aec0434e7c20c5bacaec246025


# Step 2: Generate a new address
iota client new-address ed25519 second-caller

# Step 3: Transfer coin to second-caller
# Note: replace <coin-object-id> with the actual object id of the coin you minted in step 1.
iota client transfer --to second-caller --object-id <coin-object-id>


# Step 4: Switch to second-caller
iota client switch --address second-caller

# Step 5: Get some funds to pay for the gas
iota client faucet

# Step 6: Call airdrop to get 1 coin
iota client call --package 0x7641ee891b657e349e7c34bcb0ad78bbd8ac9e41c7bcd627f822625e7b725f67 --module airdrop --function airdrop --args 0x5c154e5199c1ebbbc839c7bb19d0f44f6e82b8aec0434e7c20c5bacaec246025

# Step 7: Construct a PTB that merges the coin you received in step 1 with the coin you received in step 6, and then calls get_flag with the resulting coin.
# Note: replace <first-coin-object> and <second-coin-object> with the actual object id of the coins you received in step 1 and step 6 respectively.
iota client ptb --assign coin_1 @<first-coin-object> \
    --assign coin_2 @<second-coin-objetc> \
    --merge-coins coin_1 [coin_2]  \
    --move-call 0x7641ee891b657e349e7c34bcb0ad78bbd8ac9e41c7bcd627f822625e7b725f67::airdrop::get_flag @0x0bcae86c077ed58296e0e35e7459e3cd2722954850c8d2e6205fb415dc142bcf coin_1
