//! Command line flags.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::deck::ShuffleSource;
use crate::model::{GameType, SimConfig};
use crate::persist::ConfigStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum GameArg {
    Klondike1,
    Klondike3,
    Freecell,
    Pyramid,
    Tripeaks,
    Spider1,
    Spider2,
    Spider4,
}

impl From<GameArg> for GameType {
    fn from(game: GameArg) -> Self {
        match game {
            GameArg::Klondike1 => GameType::KlondikeDraw1,
            GameArg::Klondike3 => GameType::KlondikeDraw3,
            GameArg::Freecell => GameType::FreeCell,
            GameArg::Pyramid => GameType::Pyramid,
            GameArg::Tripeaks => GameType::TriPeaks,
            GameArg::Spider1 => GameType::Spider1Suit,
            GameArg::Spider2 => GameType::Spider2Suit,
            GameArg::Spider4 => GameType::Spider4Suit,
        }
    }
}

/// SolverLab terminal console: batch solitaire solver simulations.
///
/// Stopping a run does not interrupt games already being solved; it only
/// stops handing out new ones and discards their results.
#[derive(Debug, Parser)]
#[command(name = "solverlab-tui", version)]
pub struct Cli {
    /// Game to simulate.
    #[arg(long, value_enum)]
    pub game: Option<GameArg>,

    /// Number of games to simulate.
    #[arg(long)]
    pub sims: Option<i64>,

    /// Games played in parallel (disables auto parallelism).
    #[arg(long, conflicts_with = "auto")]
    pub parallel: Option<i64>,

    /// Adjust parallelism automatically between batches.
    #[arg(long)]
    pub auto: bool,

    /// Timeout in seconds for games that honor it (FreeCell, Pyramid, Spider).
    #[arg(long)]
    pub timeout: Option<f64>,

    /// Seed for reproducible decks across runs and platforms.
    #[arg(long)]
    pub seed: Option<u64>,

    /// Config file to load and save (defaults to the platform config dir).
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Do not read or write the config file.
    #[arg(long, conflicts_with = "config")]
    pub no_persist: bool,

    /// Use ASCII instead of Unicode glyphs.
    #[arg(long)]
    pub ascii: bool,

    /// Run without the TUI and print the CSV when done.
    #[arg(long)]
    pub headless: bool,

    /// Headless: write the CSV here instead of stdout.
    #[arg(long, value_name = "PATH", requires = "headless")]
    pub csv: Option<PathBuf>,

    /// Headless: no progress on stderr.
    #[arg(long, requires = "headless")]
    pub quiet: bool,
}

impl Cli {
    /// Headless runs only persist when a config file is given explicitly.
    pub fn config_store(&self) -> ConfigStore {
        if self.no_persist {
            ConfigStore::disabled()
        } else if let Some(path) = &self.config {
            ConfigStore::at(path)
        } else if self.headless {
            ConfigStore::disabled()
        } else {
            ConfigStore::default_path().map_or_else(ConfigStore::disabled, ConfigStore::at)
        }
    }

    pub fn shuffle(&self) -> ShuffleSource {
        self.seed
            .map_or(ShuffleSource::Random, ShuffleSource::Seeded)
    }

    pub fn apply_overrides(&self, config: &mut SimConfig) {
        if let Some(game) = self.game {
            config.game_type = game.into();
        }
        if let Some(sims) = self.sims {
            config.simulations = sims;
        }
        if let Some(parallel) = self.parallel {
            config.parallel_games = parallel;
            config.auto_parallel = false;
        }
        if self.auto {
            config.auto_parallel = true;
        }
        if let Some(timeout) = self.timeout {
            config.timeout_seconds = timeout;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("solverlab-tui").chain(args.iter().copied()))
    }

    #[test]
    fn overrides_apply_on_top_of_config() {
        let cli = parse(&[
            "--game",
            "spider2",
            "--sims",
            "40",
            "--parallel",
            "3",
            "--timeout",
            "7.5",
        ])
        .unwrap();
        let mut config = SimConfig {
            auto_parallel: true,
            ..SimConfig::default()
        };
        cli.apply_overrides(&mut config);
        assert_eq!(config.game_type, GameType::Spider2Suit);
        assert_eq!(config.simulations, 40);
        assert_eq!(config.parallel_games, 3);
        assert!(!config.auto_parallel);
        assert_eq!(config.timeout_seconds, 7.5);
    }

    #[test]
    fn no_flags_keep_config() {
        let cli = parse(&[]).unwrap();
        let mut config = SimConfig::default();
        cli.apply_overrides(&mut config);
        assert_eq!(config, SimConfig::default());
        assert_eq!(cli.shuffle(), ShuffleSource::Random);
    }

    #[test]
    fn seed_selects_seeded_shuffle() {
        assert_eq!(
            parse(&["--seed", "9"]).unwrap().shuffle(),
            ShuffleSource::Seeded(9)
        );
    }

    #[test]
    fn invalid_combinations_are_rejected() {
        assert!(parse(&["--parallel", "2", "--auto"]).is_err());
        assert!(parse(&["--csv", "out.csv"]).is_err());
        assert!(parse(&["--quiet"]).is_err());
        assert!(parse(&["--no-persist", "--config", "x.json"]).is_err());
        assert!(parse(&["--game", "solitaire"]).is_err());
    }

    #[test]
    fn headless_does_not_persist_unless_config_is_given() {
        assert!(
            parse(&["--headless"])
                .unwrap()
                .config_store()
                .path()
                .is_none()
        );
        assert!(
            parse(&["--no-persist"])
                .unwrap()
                .config_store()
                .path()
                .is_none()
        );
        let explicit = parse(&["--headless", "--config", "c.json"]).unwrap();
        assert_eq!(
            explicit.config_store().path(),
            Some(std::path::Path::new("c.json"))
        );
    }
}
