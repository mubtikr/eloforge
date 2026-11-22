use eloforge_api::{
    analyzer::{
        mistake_tagger::{tag_move, MoveContext},
        winprob::{classify, EvalSnapshot},
    },
    chess::{
        openings::detect_opening,
        pgn_importer::{parse_clock_seconds, parse_pgn},
    },
    engine::{EngineEval, EngineLine},
};

const SAMPLE: &str = r#"[Event "Live Chess"]
[Site "Chess.com"]
[Date "2025.01.02"]
[UTCDate "2025.01.02"]
[UTCTime "18:00:00"]
[EndTime "18:02:00"]
[White "TestPlayer"]
[Black "Opponent"]
[Result "1-0"]
[WhiteElo "520"]
[BlackElo "505"]
[TimeControl "600"]
[TimeClass "rapid"]
[Termination "TestPlayer won by checkmate"]

1. e4 {[%clk 0:09:58.4]} e5 {[%clk 0:09:55.1]} 2. Bc4 {[%clk 0:09:52.0]} Nc6 {[%clk 0:09:49.7]} 3. Qh5 {[%clk 0:09:47.2]} Nf6 {[%clk 0:09:44.6]} 4. Qxf7# {[%clk 0:09:40.0]} 1-0
"#;

fn engine_eval(best: &str, cp: i32, pv: Vec<&str>) -> EngineEval {
    EngineEval {
        best_move: Some(best.to_string()),
        depth: 14,
        lines: vec![EngineLine {
            multipv: 1,
            depth: 14,
            score_cp: Some(cp),
            score_mate: None,
            pv: pv.into_iter().map(str::to_string).collect(),
        }],
    }
}

#[test]
fn sample_rapid_pgn_produces_clock_and_opening_facts() {
    let parsed = parse_pgn(SAMPLE, "testplayer").expect("sample should parse");
    assert_eq!(parsed.result, "win");
    assert_eq!(parsed.moves.len(), 7);
    assert!((parsed.moves[0].time_spent_sec.unwrap_or_default() - 1.6).abs() < 0.01);

    let sans: Vec<String> = parsed.moves.iter().map(|m| m.san.clone()).collect();
    let opening = detect_opening(&sans).expect("e4 e5 Bc4 is book");
    assert!(opening.eco.starts_with('C'));
    assert!(opening.name.contains("Bishop"));
}

#[test]
fn expected_points_lost_drives_classification() {
    assert_eq!(classify(0.005, 50.0, 49.5), "best");
    assert_eq!(classify(0.10, 50.0, 40.0), "inaccuracy");
    assert_eq!(classify(0.20, 56.0, 36.0), "mistake");
    assert_eq!(classify(0.35, 52.0, 17.0), "blunder");
    assert_eq!(classify(0.10, 88.0, 54.0), "missed_win");
}

#[test]
fn deterministic_tagger_finds_a_missed_free_queen() {
    let fen_before = "7k/8/8/3q4/4Q3/8/8/7K w - - 0 1";
    let fen_after = "7k/8/8/3q4/4Q3/8/8/6K1 b - - 1 1";
    let before_eval = engine_eval("e4d5", 900, vec!["e4d5"]);
    let after_eval = engine_eval("d5e4", 900, vec!["d5e4"]);
    let before = EvalSnapshot::from_eval(&before_eval, true);
    let after = EvalSnapshot::from_eval(&after_eval, false);

    let tags = tag_move(&MoveContext {
        fen_before,
        fen_after,
        played_uci: "h1g1",
        best_uci: Some("e4d5"),
        best_pv: &["e4d5".to_string()],
        before: &before,
        after: &after,
        mover_white: true,
        epl: 0.5,
        classification: "blunder",
        phase: "endgame",
        time_spent_sec: Some(1.0),
        clock_remaining_sec: Some(580.0),
        initial_time_sec: Some(600.0),
        time_class: "rapid",
        opening_break: false,
    });

    assert!(tags.contains(&"MISSED_FREE_PIECE".to_string()));
    assert!(tags.contains(&"TACTICAL_BLINDNESS".to_string()));
}

#[test]
fn clock_parser_handles_hours_minutes_and_decimal_seconds() {
    assert_eq!(parse_clock_seconds("0:09:58.4"), Some(598.4));
    assert_eq!(parse_clock_seconds("1:00:00"), Some(3600.0));
    assert_eq!(parse_clock_seconds("0:05"), Some(5.0));
    assert_eq!(parse_clock_seconds("nonsense"), None);
}
