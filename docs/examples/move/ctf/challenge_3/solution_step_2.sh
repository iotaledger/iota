# Step 2: Merge coin_1 coin_2 coin_3 -> coin (value 6)
# Step 3: Split coin(6) -> Coin(5) + Coin(1)
# Step 4: Call get_flag with Coin(5) and counter
# Step 5: Transfer Coin(5) and Coin(1) to get flag

# Note: replace <coin-1-object-id>, <coin-2-object-id>, <coin-3-object-id> with the actual object id of the coins you minted in step 1.
# Note: replace <caller-address> with the actual address of the caller. it can be found using "iota client addresses" command.

iota client ptb --assign coin_1 @<coin-1-object-id> \
    --assign coin_2 @<coin-2-object-id> \
    --assign coin_3 @<coin-3-object-id> \
    --merge-coins coin_1 [coin_2, coin_3] \
    --split-coins coin_1 [5,1] \
    --assign my_coin \
    --move-call 0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b::mintcoin::get_flag @0xc3716689fa16bd8d8bf33ce1036b00740c8818ab9826dba846ef736501fd34b7 my_coin.0 \
    --transfer-objects [my_coin.0, my_coin.1] @<caller-address>
