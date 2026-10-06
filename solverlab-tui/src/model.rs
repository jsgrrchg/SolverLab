//! Simulation domain models, ported from `SimModels.swift`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameType {
    #[default]
    #[serde(rename = "Klondike Draw 1")]
    KlondikeDraw1,
    #[serde(rename = "Klondike Draw 3")]
    KlondikeDraw3,
    #[serde(rename = "FreeCell")]
    FreeCell,
    #[serde(rename = "Pyramid")]
    Pyramid,
    #[serde(rename = "TriPeaks")]
    TriPeaks,
    #[serde(rename = "Spider 1 Suit")]
    Spider1Suit,
    #[serde(rename = "Spider 2 Suits")]
    Spider2Suit,
    #[serde(rename = "Spider 4 Suits")]
    Spider4Suit,
}

impl GameType {
    pub const ALL: [GameType; 8] = [
        GameType::KlondikeDraw1,
        GameType::KlondikeDraw3,
        GameType::FreeCell,
        GameType::Pyramid,
        GameType::TriPeaks,
        GameType::Spider1Suit,
        GameType::Spider2Suit,
        GameType::Spider4Suit,
    ];

    /// Same as the Swift `rawValue`; used in the CSV and the default file name.
    pub fn raw_value(self) -> &'static str {
        match self {
            GameType::KlondikeDraw1 => "Klondike Draw 1",
            GameType::KlondikeDraw3 => "Klondike Draw 3",
            GameType::FreeCell => "FreeCell",
            GameType::Pyramid => "Pyramid",
            GameType::TriPeaks => "TriPeaks",
            GameType::Spider1Suit => "Spider 1 Suit",
            GameType::Spider2Suit => "Spider 2 Suits",
            GameType::Spider4Suit => "Spider 4 Suits",
        }
    }

    pub fn ui_label(self) -> &'static str {
        self.raw_value()
    }

    pub fn strategy_label(self) -> &'static str {
        match self {
            GameType::KlondikeDraw1 | GameType::KlondikeDraw3 => "IDA*",
            GameType::FreeCell => "A*",
            GameType::Pyramid => "DFS",
            GameType::TriPeaks => "A*",
            GameType::Spider1Suit | GameType::Spider2Suit | GameType::Spider4Suit => "A*",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&g| g == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Win,
    Stalled,
    Timeout,
}

impl StopReason {
    pub fn raw_value(self) -> &'static str {
        match self {
            StopReason::Win => "win",
            StopReason::Stalled => "stalled",
            StopReason::Timeout => "timeout",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StopReason::Win => "won",
            StopReason::Stalled => "stalled",
            StopReason::Timeout => "timeout",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub id: u32,
    pub move_count: i64,
    pub undo_count: i64,
    pub checkpoint_count: Option<i64>,
    pub won: bool,
    pub stop_reason: StopReason,
    /// Seconds.
    pub duration: f64,
    pub score: i64,
    pub moves_detail: String,
}

impl SimResult {
    pub fn result_label(&self) -> &'static str {
        if self.won { "won" } else { "lost" }
    }

    pub fn checkpoint_count_label(&self) -> String {
        self.checkpoint_count
            .map_or_else(|| "-".to_string(), |c| c.to_string())
    }

    pub fn checkpoint_sort_value(&self) -> i64 {
        self.checkpoint_count.unwrap_or(-1)
    }
}

/// Keys match the Swift `Codable` encoding so the JSON stays interchangeable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimConfig {
    pub game_type: GameType,
    pub simulations: i64,
    pub parallel_games: i64,
    pub auto_parallel: bool,
    pub max_undos: i64,
    pub timeout_seconds: f64,
    pub max_depth: i64,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            game_type: GameType::KlondikeDraw1,
            simulations: 0,
            parallel_games: 0,
            auto_parallel: false,
            max_undos: -1,
            timeout_seconds: 0.0,
            max_depth: 150,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchStats {
    pub completed: usize,
    pub wins: usize,
    pub timeouts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_values_and_strategies_match_swift() {
        let expected = [
            (GameType::KlondikeDraw1, "Klondike Draw 1", "IDA*"),
            (GameType::KlondikeDraw3, "Klondike Draw 3", "IDA*"),
            (GameType::FreeCell, "FreeCell", "A*"),
            (GameType::Pyramid, "Pyramid", "DFS"),
            (GameType::TriPeaks, "TriPeaks", "A*"),
            (GameType::Spider1Suit, "Spider 1 Suit", "A*"),
            (GameType::Spider2Suit, "Spider 2 Suits", "A*"),
            (GameType::Spider4Suit, "Spider 4 Suits", "A*"),
        ];
        for (game, raw, strategy) in expected {
            assert_eq!(game.raw_value(), raw);
            assert_eq!(game.ui_label(), raw);
            assert_eq!(game.strategy_label(), strategy);
        }
    }

    #[test]
    fn next_and_prev_cycle_all_games() {
        let mut game = GameType::KlondikeDraw1;
        for expected in GameType::ALL.iter().cycle().skip(1).take(8) {
            game = game.next();
            assert_eq!(game, *expected);
        }
        assert_eq!(GameType::KlondikeDraw1.prev(), GameType::Spider4Suit);
        assert_eq!(GameType::Spider4Suit.next(), GameType::KlondikeDraw1);
    }

    #[test]
    fn stop_reason_labels() {
        assert_eq!(StopReason::Win.raw_value(), "win");
        assert_eq!(StopReason::Win.label(), "won");
        assert_eq!(StopReason::Stalled.label(), "stalled");
        assert_eq!(StopReason::Timeout.raw_value(), "timeout");
    }

    #[test]
    fn default_config_matches_swift() {
        let config = SimConfig::default();
        assert_eq!(config.game_type, GameType::KlondikeDraw1);
        assert_eq!(config.simulations, 0);
        assert_eq!(config.parallel_games, 0);
        assert!(!config.auto_parallel);
        assert_eq!(config.max_undos, -1);
        assert_eq!(config.timeout_seconds, 0.0);
        assert_eq!(config.max_depth, 150);
    }

    #[test]
    fn config_serde_roundtrip_and_partial_json() {
        let config = SimConfig {
            game_type: GameType::Spider2Suit,
            simulations: 10,
            parallel_games: 3,
            auto_parallel: true,
            max_undos: 5,
            timeout_seconds: 12.5,
            max_depth: 88,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"gameType\":\"Spider 2 Suits\""));
        assert!(json.contains("\"parallelGames\":3"));
        assert_eq!(serde_json::from_str::<SimConfig>(&json).unwrap(), config);

        let partial: SimConfig = serde_json::from_str(r#"{"gameType":"FreeCell"}"#).unwrap();
        assert_eq!(partial.game_type, GameType::FreeCell);
        assert_eq!(partial.max_depth, 150);
        assert_eq!(partial.max_undos, -1);
    }

    #[test]
    fn result_labels() {
        let mut result = SimResult {
            id: 1,
            move_count: 0,
            undo_count: 0,
            checkpoint_count: None,
            won: false,
            stop_reason: StopReason::Stalled,
            duration: 0.0,
            score: 0,
            moves_detail: String::new(),
        };
        assert_eq!(result.result_label(), "lost");
        assert_eq!(result.checkpoint_count_label(), "-");
        assert_eq!(result.checkpoint_sort_value(), -1);
        result.won = true;
        result.checkpoint_count = Some(4);
        assert_eq!(result.result_label(), "won");
        assert_eq!(result.checkpoint_count_label(), "4");
        assert_eq!(result.checkpoint_sort_value(), 4);
    }
}
