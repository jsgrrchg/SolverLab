//! CSV export, byte-for-byte compatible with `SimulationViewModel.makeCSV`.

use chrono::{DateTime, TimeZone};

use crate::model::{GameType, SimConfig, SimResult, StopReason};

pub fn csv_escape(raw: &str) -> String {
    format!("\"{}\"", raw.replace('"', "\"\""))
}

pub fn make_csv(
    rows: &[SimResult],
    timeout_active_display: &str,
    run_config: Option<&SimConfig>,
) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 12);

    lines.push("configuration".into());
    lines.push(
        "game,search,simulations,parallel_games,auto_parallel,undos,timeout_s,max_depth".into(),
    );
    match run_config {
        Some(config) => lines.push(format!(
            "{},{},{},{},{},{},{:.2},{}",
            csv_escape(config.game_type.raw_value()),
            csv_escape(config.game_type.strategy_label()),
            config.simulations,
            config.parallel_games,
            config.auto_parallel,
            config.max_undos,
            config.timeout_seconds,
            config.max_depth
        )),
        None => lines.push("\"(no_config)\",\"(no_config)\",0,0,false,0,0.00,0".into()),
    }
    lines.push(String::new());

    let mut sorted: Vec<&SimResult> = rows.iter().collect();
    sorted.sort_by_key(|row| row.id);
    let total_games = sorted.len();
    let count = |reason: StopReason| sorted.iter().filter(|r| r.stop_reason == reason).count();
    let win_count = count(StopReason::Win);
    let stalled_count = count(StopReason::Stalled);
    let timeout_count = count(StopReason::Timeout);
    let total_moves: i64 = sorted.iter().map(|r| r.move_count).sum();
    let total_undos: i64 = sorted.iter().map(|r| r.undo_count).sum();
    let total_duration: f64 = sorted.iter().map(|r| r.duration).sum();
    let total_checkpoints: i64 = sorted.iter().map(|r| r.checkpoint_count.unwrap_or(0)).sum();
    let average = |total: f64| {
        if total_games > 0 {
            total / total_games as f64
        } else {
            0.0
        }
    };
    let win_rate = average(win_count as f64) * 100.0;

    lines.push("summary".into());
    lines.push(
        "games,active_timeout,win_rate,avg_moves,avg_undos,avg_checkpoints,avg_duration,wins,stalled,timeouts"
            .into(),
    );
    lines.push(format!(
        "{total_games},{timeout_active_display},{win_rate:.2}%,{:.2},{:.2},{:.2},{:.2}s,{win_count},{stalled_count},{timeout_count}",
        average(total_moves as f64),
        average(total_undos as f64),
        average(total_checkpoints as f64),
        average(total_duration),
    ));
    lines.push(String::new());

    lines.push("details".into());
    lines.push("game_id,moves,undos,checkpoints,result,stop_reason,duration_sec,score".into());
    for row in sorted {
        let checkpoints = row
            .checkpoint_count
            .map(|c| c.to_string())
            .unwrap_or_default();
        lines.push(format!(
            "{},{},{},{},{},{},{:.3},{}",
            row.id,
            row.move_count,
            row.undo_count,
            checkpoints,
            row.result_label(),
            row.stop_reason.raw_value(),
            row.duration,
            row.score
        ));
    }

    lines.join("\n") + "\n"
}

/// `{GameName}_results_{yyyyMMdd_HHmmss}.csv`, where `GameName` keeps only the
/// alphanumeric tokens of the game's raw value.
pub fn default_csv_file_name<Tz: TimeZone>(game: GameType, now: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let name: String = game
        .raw_value()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let name = if name.is_empty() { "Game".into() } else { name };
    format!("{name}_results_{}.csv", now.format("%Y%m%d_%H%M%S"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[allow(clippy::too_many_arguments)]
    fn result(
        id: u32,
        moves: i64,
        undos: i64,
        checkpoints: Option<i64>,
        won: bool,
        stop_reason: StopReason,
        duration: f64,
        score: i64,
    ) -> SimResult {
        SimResult {
            id,
            move_count: moves,
            undo_count: undos,
            checkpoint_count: checkpoints,
            won,
            stop_reason,
            duration,
            score,
            moves_detail: String::new(),
        }
    }

    #[test]
    fn csv_sorts_rows_summarizes_and_escapes_config() {
        let rows = [
            result(2, 20, 4, None, false, StopReason::Timeout, 3.5, 7),
            result(1, 10, 2, Some(6), true, StopReason::Win, 1.5, 9),
        ];
        let config = SimConfig {
            game_type: GameType::Spider2Suit,
            simulations: 2,
            parallel_games: 3,
            auto_parallel: true,
            max_undos: 5,
            timeout_seconds: 12.345,
            max_depth: 88,
        };

        let csv = make_csv(&rows, "00:12", Some(&config));

        assert!(csv.contains("\"Spider 2 Suits\",\"A*\",2,3,true,5,12.35,88"));
        assert!(csv.contains("2,00:12,50.00%,15.00,3.00,3.00,2.50s,1,0,1"));
        let first = csv.find("\n1,10,2,6,won,win,1.500,9").unwrap();
        let second = csv.find("\n2,20,4,,lost,timeout,3.500,7").unwrap();
        assert!(first < second);
        assert_eq!(csv_escape("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn csv_without_config_or_rows() {
        let csv = make_csv(&[], "00:00", None);
        assert_eq!(
            csv,
            "configuration\n\
             game,search,simulations,parallel_games,auto_parallel,undos,timeout_s,max_depth\n\
             \"(no_config)\",\"(no_config)\",0,0,false,0,0.00,0\n\
             \n\
             summary\n\
             games,active_timeout,win_rate,avg_moves,avg_undos,avg_checkpoints,avg_duration,wins,stalled,timeouts\n\
             0,00:00,0.00%,0.00,0.00,0.00,0.00s,0,0,0\n\
             \n\
             details\n\
             game_id,moves,undos,checkpoints,result,stop_reason,duration_sec,score\n"
        );
    }

    #[test]
    fn default_file_name_strips_non_alphanumerics() {
        let now = Utc.with_ymd_and_hms(2026, 10, 6, 9, 5, 7).unwrap();
        assert_eq!(
            default_csv_file_name(GameType::Spider2Suit, now),
            "Spider2Suits_results_20261006_090507.csv"
        );
        assert_eq!(
            default_csv_file_name(GameType::KlondikeDraw1, now),
            "KlondikeDraw1_results_20261006_090507.csv"
        );
    }
}
