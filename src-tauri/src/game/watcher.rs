use crate::{
    db::map_rank_row,
    domain::{ChampionRankRow, GamePhase, GameState},
    AppState,
};
use reqwest::{Client, StatusCode};
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::{path::Path, time::Duration};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};
use tauri::{AppHandle, Emitter, Manager, State};

const DETECTION_SETTING: &str = "game_detection_enabled";

struct LcuCredentials {
    port: String,
    token: String,
}

struct Observation {
    connected: bool,
    phase: GamePhase,
    source: Option<String>,
    champion_id: Option<String>,
    champion_name_hint: Option<String>,
    team: Vec<Option<ObservedChampion>>,
    bench: Vec<ObservedChampion>,
}

struct ObservedChampion {
    id: String,
    name_hint: Option<String>,
}

#[tauri::command]
pub async fn get_game_state(state: State<'_, AppState>) -> Result<GameState, String> {
    read_game_state(&state).await
}

async fn read_game_state(state: &AppState) -> Result<GameState, String> {
    let enabled = {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库锁定失败".to_string())?;
        match db.query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [DETECTION_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        {
            Ok(Some(value)) => value == "true",
            Ok(None) => true,
            Err(error) => return Err(format!("读取客户端检测设置失败：{error}")),
        }
    };

    if !enabled {
        if let Ok(mut ocr) = state.ocr.lock() {
            ocr.shutdown_if_idle(Duration::from_secs(300));
        }
        return Ok(GameState {
            enabled: false,
            connected: false,
            phase: GamePhase::Disconnected,
            source: None,
            player_champion_id: None,
            player_champion_name: None,
            ranking: None,
            team: Vec::new(),
            bench: Vec::new(),
        });
    }

    let observation = observe_game().await;
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let champion = match observation.champion_id.as_deref() {
        Some(id) => resolve_champion(&db, id, observation.champion_name_hint.as_deref())?,
        None => None,
    };

    let bench = observation.bench.iter().map(|entry| resolve_champion(&db, &entry.id, entry.name_hint.as_deref()).map(|row| row.and_then(|(_, _, rank)| rank))).collect::<Result<Vec<_>, _>>()?;
    let team = observation.team.iter().map(|entry| match entry {
        Some(entry) => resolve_champion(&db, &entry.id, entry.name_hint.as_deref())
            .map(|row| row.and_then(|(_, _, rank)| rank)),
        None => Ok(None),
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(GameState {
        enabled: true,
        connected: observation.connected,
        phase: observation.phase,
        source: observation.source,
        player_champion_id: champion
            .as_ref()
            .map(|(id, _, _)| id.clone())
            .or(observation.champion_id),
        player_champion_name: champion.as_ref().map(|(_, name, _)| name.clone()),
        ranking: champion.and_then(|(_, _, ranking)| ranking),
        team,
        bench,
    })
}

pub fn start_game_state_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut previous: Option<GameState> = None;
        loop {
            let state = app.state::<AppState>();
            match read_game_state(&state).await {
                Ok(current) => {
                    if previous.as_ref() != Some(&current) {
                        previous = Some(current.clone());
                        let _ = app.emit_to("main", "game:state", current.clone());
                    }
                    let _ = crate::commands::game::show_champion_rank_overlay(current, app.clone());
                }
                Err(error) => eprintln!("读取游戏客户端状态失败：{error}"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn observe_game() -> Observation {
    if let Some(credentials) = find_lcu_credentials() {
        if let Some(observation) = observe_lcu(credentials).await {
            return observation;
        }
    }
    observe_live_client().await.unwrap_or(Observation {
        connected: false,
        phase: GamePhase::Disconnected,
        source: None,
        champion_id: None,
        champion_name_hint: None,
        team: Vec::new(),
        bench: Vec::new(),
    })
}

fn find_lcu_credentials() -> Option<LcuCredentials> {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    )
    .ok()?;
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let process_ids = system
        .processes()
        .iter()
        .filter(|(_, process)| {
            let name = process.name().to_string_lossy();
            ["LeagueClientUx.exe", "LeagueClientUx", "LeagueClient.exe", "LeagueClient"]
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
        .map(|(pid, _)| *pid)
        .collect::<Vec<_>>();

    for pid in process_ids {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
        );
        let Some(process) = system.process(pid) else {
            continue;
        };
        let executable = process.exe().map(Path::to_path_buf);
        let arguments = process
            .cmd()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if let Some(credentials) = parse_lcu_credentials(&arguments) {
            return Some(credentials);
        }

        #[cfg(windows)]
        if let Some(command_line) = read_process_command_line(pid.as_u32()) {
            let arguments = command_line
                .split_whitespace()
                .map(|argument| argument.trim_matches('"').to_string())
                .collect::<Vec<_>>();
            if let Some(credentials) = parse_lcu_credentials(&arguments) {
                return Some(credentials);
            }
        }

        if let Some(executable) = executable {
            if let Some(credentials) = read_lcu_lockfile(&executable) {
                return Some(credentials);
            }
        }
    }
    None
}

fn parse_lcu_credentials(arguments: &[String]) -> Option<LcuCredentials> {
    let mut port = None;
    let mut token = None;
    let mut args = arguments.iter();
    while let Some(argument) = args.next() {
        let argument = argument.trim_matches('"');
        if let Some(value) = argument.strip_prefix("--app-port=") {
            port = Some(value.trim_matches('"').to_string());
        } else if argument == "--app-port" {
            port = args.next().map(|value| value.trim_matches('"').to_string());
        } else if let Some(value) = argument.strip_prefix("--remoting-auth-token=") {
            token = Some(value.trim_matches('"').to_string());
        } else if argument == "--remoting-auth-token" {
            token = args.next().map(|value| value.trim_matches('"').to_string());
        }
    }
    let (port, token) = (port?, token?);
    if port.parse::<u16>().is_ok_and(|value| value > 0) && !token.is_empty() {
        Some(LcuCredentials { port, token })
    } else {
        None
    }
}

fn read_lcu_lockfile(executable: &Path) -> Option<LcuCredentials> {
    let mut directory = executable.parent()?;
    for _ in 0..=4 {
        let path = directory.join("lockfile");
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Some(credentials) = parse_lcu_lockfile(&contents) {
                return Some(credentials);
            }
        }
        directory = directory.parent()?;
    }
    None
}

fn parse_lcu_lockfile(contents: &str) -> Option<LcuCredentials> {
    let mut fields = contents.trim().split(':');
    let process = fields.next()?;
    let pid = fields.next()?.parse::<u32>().ok()?;
    let port = fields.next()?;
    let token = fields.next()?;
    let protocol = fields.next()?;
    let port = port.parse::<u16>().ok()?;
    if process.is_empty() || pid == 0 || port == 0 || token.is_empty() || protocol != "https" {
        return None;
    }
    Some(LcuCredentials {
        port: port.to_string(),
        token: token.to_string(),
    })
}

#[cfg(windows)]
fn read_process_command_line(pid: u32) -> Option<String> {
    use std::{
        ffi::c_void,
        mem::size_of,
        slice,
    };

    type Handle = *mut c_void;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
    const COMMAND_LINE_BUFFER_SIZE: usize = 64 * 1024;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *const u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process: Handle,
            information_class: u32,
            information: *mut c_void,
            information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }

    let mut buffer = vec![0usize; COMMAND_LINE_BUFFER_SIZE.div_ceil(size_of::<usize>())];
    let buffer_start = buffer.as_mut_ptr().cast::<c_void>();
    let buffer_length = (buffer.len() * size_of::<usize>()) as u32;
    let mut return_length = 0;
    let status = unsafe {
        NtQueryInformationProcess(
            process,
            PROCESS_COMMAND_LINE_INFORMATION,
            buffer_start,
            buffer_length,
            &mut return_length,
        )
    };
    unsafe { CloseHandle(process) };
    if status < 0 {
        return None;
    }

    let command_line = unsafe { buffer_start.cast::<UnicodeString>().read_unaligned() };
    let start = buffer_start as usize;
    let end = start.checked_add(buffer_length as usize)?;
    let command_start = command_line.buffer as usize;
    let command_end = command_start.checked_add(command_line.length as usize)?;
    if command_line.length == 0
        || command_line.length % size_of::<u16>() as u16 != 0
        || command_start < start
        || command_end > end
    {
        return None;
    }

    let utf16 = unsafe {
        slice::from_raw_parts(command_line.buffer, command_line.length as usize / size_of::<u16>())
    };
    Some(String::from_utf16_lossy(utf16))
}

async fn observe_lcu(credentials: LcuCredentials) -> Option<Observation> {
    let client = Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let base_url = format!("https://127.0.0.1:{}", credentials.port);
    let phase_response = client
        .get(format!("{base_url}/lol-gameflow/v1/gameflow-phase"))
        .basic_auth("riot", Some(credentials.token.as_str()))
        .send()
        .await
        .ok()?;
    if !phase_response.status().is_success() {
        return None;
    }
    let phase_text = phase_response.text().await.ok()?;
    let raw_phase = serde_json::from_str::<String>(&phase_text).unwrap_or(phase_text);
    let phase = match raw_phase.as_str() {
        "ChampSelect" => GamePhase::ChampSelect,
        "GameStart" => GamePhase::Loading,
        "InProgress" => GamePhase::InGame,
        "ReadyCheck" | "Lobby" | "Matchmaking" | "EndOfGame" | "WaitingForStats" | "None" => {
            GamePhase::Disconnected
        }
        _ => GamePhase::Disconnected,
    };
    let (champion, bench, team) = if matches!(phase, GamePhase::ChampSelect) {
        lcu_champ_select_champion(&client, &base_url, &credentials.token).await.unwrap_or_default()
    } else if matches!(phase, GamePhase::InGame | GamePhase::Loading) {
        (live_client_champion(&client).await, Vec::new(), Vec::new())
    } else {
        (None, Vec::new(), Vec::new())
    };

    Some(Observation {
        connected: true,
        phase,
        source: Some("lcu".to_string()),
        champion_id: champion.as_ref().map(|champion| champion.id.clone()),
        champion_name_hint: champion.and_then(|champion| champion.name_hint),
        team,
        bench,
    })
}

fn team_champion_ids(session: &Value) -> Vec<Option<i64>> {
    session.get("myTeam").and_then(Value::as_array).map(|players| {
        players.iter().take(5).map(|player| {
            player.get("championId").and_then(Value::as_i64).filter(|id| *id > 0)
                .or_else(|| player.get("championPickIntent").and_then(Value::as_i64).filter(|id| *id > 0))
        }).collect()
    }).unwrap_or_default()
}

async fn lcu_champ_select_champion(
    client: &Client,
    base_url: &str,
    token: &str,
) -> Option<(Option<ObservedChampion>, Vec<ObservedChampion>, Vec<Option<ObservedChampion>>)> {
    let response = client
        .get(format!("{base_url}/lol-champ-select/v1/session"))
        .basic_auth("riot", Some(token))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let session: Value = response.json().await.ok()?;
    let local_cell = session.get("localPlayerCellId")?.as_i64()?;
    let player = session
        .get("myTeam")?
        .as_array()?
        .iter()
        .find(|player| player.get("cellId").and_then(Value::as_i64) == Some(local_cell))?;
    let selected = player
        .get("championId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .or_else(|| {
            player
                .get("championPickIntent")
                .and_then(Value::as_i64)
                .filter(|id| *id > 0)
        });
    let summaries: Value = client.get(format!("{base_url}/lol-game-data/assets/v1/champion-summary.json"))
        .basic_auth("riot", Some(token)).send().await.ok()?.json().await.ok()?;
    let champion = selected.and_then(|id| lcu_champion_alias(&summaries, id));
    let mut bench = Vec::new();
    if let Some(rows) = session.get("benchChampions").or_else(|| session.get("benchChampionIds")).and_then(Value::as_array) {
        for row in rows.iter().take(10) {
            let id = row.as_i64().or_else(|| row.get("championId").and_then(Value::as_i64)).unwrap_or(0);
            let entry = if id > 0 { lcu_champion_alias(&summaries, id) } else { None };
            bench.push(entry.unwrap_or(ObservedChampion { id: format!("unknown{id}"), name_hint: None }));
        }
    }
    let team = team_champion_ids(&session).into_iter()
        .map(|id| id.and_then(|id| lcu_champion_alias(&summaries, id))).collect();
    Some((champion, bench, team))
}

fn lcu_champion_alias(champions: &Value, champion_id: i64) -> Option<ObservedChampion> {
    let champion = champions
        .as_array()?
        .iter()
        .find(|champion| champion.get("id").and_then(Value::as_i64) == Some(champion_id))?;
    let alias = champion.get("alias").and_then(Value::as_str)?;
    let name_hint = champion
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    Some(ObservedChampion {
        id: alias.to_string(),
        name_hint,
    })
}

async fn observe_live_client() -> Option<Observation> {
    let client = Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;
    let champion = live_client_champion(&client).await?;
    Some(Observation {
        connected: true,
        phase: GamePhase::InGame,
        source: Some("live_client".to_string()),
        champion_id: Some(champion.id),
        champion_name_hint: champion.name_hint,
        team: Vec::new(),
        bench: Vec::new(),
    })
}

async fn live_client_champion(client: &Client) -> Option<ObservedChampion> {
    let response = client
        .get("https://127.0.0.1:2999/liveclientdata/activeplayer")
        .send()
        .await
        .ok()?;
    if response.status() != StatusCode::OK {
        return None;
    }
    let active: Value = response.json().await.ok()?;
    let players: Value = client.get("https://127.0.0.1:2999/liveclientdata/playerlist")
        .send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    champion_from_live_data(&active, &players)
}

fn champion_from_live_data(active: &Value, players: &Value) -> Option<ObservedChampion> {
    let players = players.as_array()?;
    let player = ["riotId", "summonerName"].into_iter().find_map(|key| {
        let value = active.get(key)?.as_str()?.trim();
        if value.is_empty() { return None; }
        players.iter().find(|player| player.get(key).and_then(Value::as_str) == Some(value))
    })?;
    let name = player.get("championName").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let alias = player.get("rawChampionName").and_then(Value::as_str)
        .and_then(|raw| raw.strip_prefix("game_character_displayname_"))
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric()));
    Some(ObservedChampion { id: alias.or(name)?.to_string(), name_hint: name.map(str::to_string) })
}

fn resolve_champion(
    db: &rusqlite::Connection,
    champion_id: &str,
    name_hint: Option<&str>,
) -> Result<Option<(String, String, Option<ChampionRankRow>)>, String> {
    let champion = crate::db::resolve_champion_identity(db, champion_id, name_hint)
        .map_err(|error| format!("查询当前英雄失败：{error}"))?;
    let Some((resolved_id, name)) = champion else {
        return Ok(Some((
            champion_id.to_string(),
            name_hint.unwrap_or(champion_id).to_string(),
            None,
        )));
    };
    let mut ranking = db
        .query_row(
            "SELECT champion_id, name, position, total, badge_tier, patch, source, updated_at FROM champion_rankings WHERE champion_id = ?1",
            [&resolved_id],
            map_rank_row,
        )
        .optional()
        .map_err(|error| format!("查询当前英雄排名失败：{error}"))?;
    if let Some(rank) = ranking.as_mut().filter(|r| r.source.as_deref().is_some_and(|s| s.starts_with("AramHex"))) {
        let has_cache: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='aramhex_heroes')", [], |row| row.get(0))
            .map_err(|error| format!("查询胜率缓存失败：{error}"))?;
        if has_cache {
            rank.win_rate = db.query_row("SELECT win_rate FROM aramhex_heroes WHERE champion_id=?1", [&resolved_id], |row| row.get::<_, f64>(0))
                .optional().map_err(|error| format!("查询英雄胜率失败：{error}"))?
                .filter(|rate| rate.is_finite() && (0.0..=1.0).contains(rate));
        }
    }
    Ok(Some((resolved_id, name, ranking)))
}

#[cfg(test)]
mod tests {
    use super::{champion_from_live_data, parse_lcu_credentials, parse_lcu_lockfile};

    #[test]
    fn reads_cached_overall_rate_only_for_matching_source() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE champions (champion_id TEXT, name TEXT);
            CREATE TABLE aramhex_heroes (champion_id TEXT, source_id TEXT, name TEXT, title TEXT, win_rate REAL);
            CREATE TABLE champion_rankings (champion_id TEXT, name TEXT, position INTEGER, total INTEGER, badge_tier INTEGER, patch TEXT, source TEXT, updated_at TEXT);
            INSERT INTO champions VALUES ('Ahri','Ahri');
            INSERT INTO aramhex_heroes VALUES ('Ahri','103','Ahri','Ahri',0.523);
            INSERT INTO champion_rankings VALUES ('Ahri','Ahri',10,170,NULL,'16.21','AramHex',NULL);").unwrap();
        let rate = || super::resolve_champion(&db, "103", None).unwrap().unwrap().2.unwrap().win_rate;
        assert_eq!(rate(), Some(0.523));
        db.execute("UPDATE aramhex_heroes SET win_rate=2", []).unwrap();
        assert_eq!(rate(), None);
        db.execute("UPDATE aramhex_heroes SET win_rate=0.523", []).unwrap();
        db.execute("UPDATE champion_rankings SET source='Other'", []).unwrap();
        assert_eq!(rate(), None);
        db.execute("DELETE FROM aramhex_heroes", []).unwrap();
        assert!(super::resolve_champion(&db, "Ahri", None).unwrap().unwrap().2.unwrap().win_rate.is_none());
    }

    #[test]
    fn team_selection_preserves_empty_slots_and_updates_swaps() {
        let mut session = serde_json::json!({"myTeam": [
            {"championId": 103}, {"championId": 0},
            {"championId": 17}, {"championPickIntent": 22}, {"championId": 99}
        ]});
        assert_eq!(super::team_champion_ids(&session), vec![Some(103), None, Some(17), Some(22), Some(99)]);
        session["myTeam"][0]["championId"] = serde_json::json!(17);
        assert_eq!(super::team_champion_ids(&session)[0], Some(17));
        assert!(super::team_champion_ids(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn finds_local_players_champion_from_game_client_data() {
        let active = serde_json::json!({"riotId": "Example#123"});
        let players = serde_json::json!([
            {"riotId":"Other#456", "championName":"提莫", "rawChampionName":"game_character_displayname_Teemo"},
            {"riotId":"Example#123", "championName":"阿狸", "rawChampionName":"game_character_displayname_Ahri"}
        ]);
        let champion = champion_from_live_data(&active, &players).unwrap();
        assert_eq!(champion.id, "Ahri");
        assert_eq!(champion.name_hint.as_deref(), Some("阿狸"));
        assert!(champion_from_live_data(&serde_json::json!({"riotId":"missing"}), &players).is_none());
    }

    #[test]
    fn parses_lcu_lockfile_credentials() {
        let credentials = parse_lcu_lockfile("LeagueClient:1234:54321:local-secret:https").unwrap();
        assert_eq!(credentials.port, "54321");
        assert_eq!(credentials.token, "local-secret");
    }

    #[test]
    fn rejects_invalid_lcu_lockfile_credentials() {
        assert!(parse_lcu_lockfile("LeagueClient:no-pid:54321:local-secret:https").is_none());
        assert!(parse_lcu_lockfile("LeagueClient:1234:0:local-secret:https").is_none());
        assert!(parse_lcu_lockfile("LeagueClient:1234:54321::http").is_none());
    }

    #[test]
    fn parses_credentials_from_equals_and_separate_arguments() {
        let equals = [
            "--app-port=54321".to_string(),
            "--remoting-auth-token=local-secret".to_string(),
        ];
        let separate = [
            "--app-port".to_string(),
            "54321".to_string(),
            "--remoting-auth-token".to_string(),
            "local-secret".to_string(),
        ];
        assert_eq!(parse_lcu_credentials(&equals).unwrap().port, "54321");
        assert_eq!(parse_lcu_credentials(&separate).unwrap().token, "local-secret");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::{find_lcu_credentials, observe_lcu};

    #[test]
    #[ignore = "requires a running LeagueClientUx process"]
    fn finds_credentials_from_running_lcu_client() {
        let credentials = find_lcu_credentials()
            .expect("the running League client should expose local API credentials");
        let observation = tauri::async_runtime::block_on(observe_lcu(credentials))
            .expect("the running League client should respond to its local API");
        eprintln!(
            "LCU connected: phase={:?}; champion selected/intended={}",
            observation.phase,
            observation.champion_id.is_some()
        );
        assert!(observation.connected);
    }
}
