module ctf::verifier {

    use std::ascii::{String};
    use std::type_name::{get, into_string};
    use iota::table::{Self, Table};
    use iota::clock::{Self, Clock};
    use iota::address;
    use iota::event;

    public struct CapturedFlag has store {
        flag_type: String,
        capture_time: u64
    }

    public struct Score has store {
        captured_flags: u64,
        latest_capture: u64
    }

    public struct ScoreBoard has copy, drop {
        scores: vector<ScoreBoardElement>
    }

    public struct ScoreBoardElement has copy, drop {
        sender: address,
        captured_flags: u64,
        latest_capture: u64
    }

    public struct Challenges has key {
        id: UID,
        /// The type name of the flags that are allowed to be submitted
        allowed_flags: vector<String>,
        captured_flags: Table<address, vector<CapturedFlag>>,
        participants: vector<address>,
        scores: Table<address, Score>
    }

    public struct CTFCap has key, store {
        id: UID
    }

    const EFlagNotAllowed: u64 = 0;
    const EFlagAlreadySubmitted: u64 = 1;

    fun init (ctx: &mut TxContext) {
        transfer::public_transfer(
            CTFCap {
                id: object::new(ctx)
            }, tx_context::sender(ctx)
        );

        transfer::share_object(
            Challenges {
                id: object::new(ctx),
                allowed_flags: vector::empty<String>(),
                captured_flags: table::new<address, vector<CapturedFlag>>(ctx),
                participants: vector::empty<address>(),
                scores: table::new<address, Score>(ctx)
            }
        );
    }

    /// Admin functionality to enable a new flag type to be submitted
    public fun allow_flag_type<T>(_cap: &CTFCap, challenges: &mut Challenges) {
        let flag_type_string = get<T>().into_string();
        challenges.allowed_flags.push_back(flag_type_string);
    }   

    /// Capture function to call after sending the flag here!
    public fun capture<T: key + store>(challenges: &mut Challenges, captured_flag: T, clock: &Clock, ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);

        let tname = get<T>().into_string();

        // Check if this flag is allowed to be submitted
        assert!(vector::contains(&challenges.allowed_flags, &tname), EFlagNotAllowed);

        let cflag = CapturedFlag { flag_type: tname, capture_time: clock::timestamp_ms(clock)};

        // Add the empty vector for non-existing flags
        if(!table::contains(&challenges.captured_flags, sender)) {
            table::add(&mut challenges.captured_flags, sender, vector::empty<CapturedFlag>());
        };

        // Initialize a scoreboard object for this sender if not there yet        
        if(!table::contains(&challenges.scores, sender)) {
            table::add(&mut challenges.scores, sender, Score { captured_flags: 0, latest_capture: 0});
            challenges.participants.push_back(sender);
        };

        // Check if flag was already captured
        let mut i = 0;
        let submitted = table::borrow(&challenges.captured_flags, sender);

        while(i < submitted.length()) {
            let tmp_cflag = &submitted[i];
            assert!(tmp_cflag.flag_type != tname, EFlagAlreadySubmitted);
            i = i + 1;
        };

        let captured_flags = table::borrow_mut(&mut challenges.captured_flags, sender);
        captured_flags.push_back(cflag);

        let scoreboard = table::borrow_mut(&mut challenges.scores, sender);
        scoreboard.captured_flags = scoreboard.captured_flags + 1;
        scoreboard.latest_capture = clock::timestamp_ms(clock);

        // Burn the flag!  
        transfer::public_transfer(captured_flag, address::from_u256(1337));
    }

    /// Return the global scoreboard
    public fun scoreboard(challenges: &Challenges): vector<ScoreBoardElement> {
        let mut i = 0;
        let mut score_vector = vector::empty<ScoreBoardElement>();

        while(i < challenges.participants.length()) {
            let addr = challenges.participants[i];
            let score = &challenges.scores[addr];
            score_vector.push_back(ScoreBoardElement { sender: addr, captured_flags: score.captured_flags, latest_capture: score.latest_capture });
            i = i + 1;
        };

        event::emit(ScoreBoard { scores: score_vector });
        score_vector
    }

    /// Return the captures for a given address
    public fun captured(challenges: &Challenges, addr: address): &vector<CapturedFlag> {
        table::borrow(&challenges.captured_flags, addr)
    }

}
