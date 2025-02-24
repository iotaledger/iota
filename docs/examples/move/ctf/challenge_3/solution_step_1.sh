# Step 1: Call mint_coin 3 times to have 3 coin object of value 2


iota client ptb --move-call 0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b::mintcoin::mint_coin @0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741 \
    --move-call 0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b::mintcoin::mint_coin @0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741 \
    --move-call 0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b::mintcoin::mint_coin @0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741
