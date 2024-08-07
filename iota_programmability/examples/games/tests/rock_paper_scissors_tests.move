// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module games::rock_paper_scissors_tests {
    use iota::test_scenario::Self;
    use std::hash;
    use games::rock_paper_scissors::status;
    use games::rock_paper_scissors::Game;
    use games::rock_paper_scissors::ThePrize;
    use games::rock_paper_scissors::select_winner;
    use games::rock_paper_scissors::match_secret;
    use games::rock_paper_scissors::Secret;
    use games::rock_paper_scissors::reveal;
    use games::rock_paper_scissors::add_hash;
    use games::rock_paper_scissors::PlayerTurn;
    use games::rock_paper_scissors::player_turn;
    use games::rock_paper_scissors::scissors;
    use games::rock_paper_scissors::new_game;
    use games::rock_paper_scissors::rock;

    #[test]
    fun play_rock_paper_scissors() {
        // So these are our heroes.
        let the_main_guy = @0xA1C05;
        let mr_lizard = @0xA55555;
        let mr_spock = @0x590C;

        let mut scenario_val = test_scenario::begin(the_main_guy);
        let scenario = &mut scenario_val;

        // Let the game begin!
        new_game(mr_spock, mr_lizard, test_scenario::ctx(scenario));

        // Mr Spock makes his move. He does it secretly and hashes the gesture with a salt
        // so that only he knows what it is.
        test_scenario::next_tx(scenario, mr_spock);
        {
            let hash = hash(rock(), b"my_phaser_never_failed_me!");
            player_turn(the_main_guy, hash, test_scenario::ctx(scenario));
        };

        // Now it's time for The Main Guy to accept his turn.
        test_scenario::next_tx(scenario, the_main_guy);
        {
            let mut game = test_scenario::take_from_sender<Game>(scenario);
            let cap = test_scenario::take_from_sender<PlayerTurn>(scenario);

            assert!(status(&game) == 0, 0); // STATUS_READY

            add_hash(&mut game, cap);

            assert!(status(&game) == 1, 0); // STATUS_HASH_SUBMISSION

            test_scenario::return_to_sender(scenario, game);
        };

        // Same for Mr Lizard. He uses his secret phrase to encode his turn.
        test_scenario::next_tx(scenario, mr_lizard);
        {
            let hash = hash(scissors(), b"sssssss_you_are_dead!");
            player_turn(the_main_guy, hash, test_scenario::ctx(scenario));
        };

        test_scenario::next_tx(scenario, the_main_guy);
        {
            let mut game = test_scenario::take_from_sender<Game>(scenario);
            let cap = test_scenario::take_from_sender<PlayerTurn>(scenario);
            add_hash(&mut game, cap);

            assert!(status(&game) == 2, 0); // STATUS_HASHES_SUBMITTED

            test_scenario::return_to_sender(scenario, game);
        };

        // Now that both sides made their moves, it's time for  Mr Spock and Mr Lizard to
        // reveal their secrets. The Main Guy will then be able to determine the winner. Who's
        // gonna win The Prize? We'll see in a bit!
        test_scenario::next_tx(scenario, mr_spock);
        reveal(the_main_guy, b"my_phaser_never_failed_me!", test_scenario::ctx(scenario));

        test_scenario::next_tx(scenario, the_main_guy);
        {
            let mut game = test_scenario::take_from_sender<Game>(scenario);
            let secret = test_scenario::take_from_sender<Secret>(scenario);
            match_secret(&mut game, secret);

            assert!(status(&game) == 3, 0); // STATUS_REVEALING

            test_scenario::return_to_sender(scenario, game);
        };

        test_scenario::next_tx(scenario, mr_lizard);
        reveal(the_main_guy, b"sssssss_you_are_dead!", test_scenario::ctx(scenario));

        // The final step. The Main Guy matches and reveals the secret of the Mr Lizard and
        // calls the [`select_winner`] function to release The Prize.
        test_scenario::next_tx(scenario, the_main_guy);
        {
            let mut game = test_scenario::take_from_sender<Game>(scenario);
            let secret = test_scenario::take_from_sender<Secret>(scenario);
            match_secret(&mut game, secret);

            assert!(status(&game) == 4, 0); // STATUS_REVEALED

            select_winner(game, test_scenario::ctx(scenario));
        };

        test_scenario::next_tx(scenario, mr_spock);
        // If it works, then MrSpock is in possession of the prize;
        let prize = test_scenario::take_from_sender<ThePrize>(scenario);
        // Don't forget to give it back!
        test_scenario::return_to_sender(scenario, prize);
        test_scenario::end(scenario_val);
    }

    // Copy of the hashing function from the main module.
    fun hash(gesture: u8, mut salt: vector<u8>): vector<u8> {
        vector::push_back(&mut salt, gesture);
        hash::sha2_256(salt)
    }
}
