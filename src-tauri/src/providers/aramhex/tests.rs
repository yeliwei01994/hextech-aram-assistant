use super::*;

#[test]
fn upgrading_old_hero_table_backfills_names_without_replacing_cached_statistics() {
    for existing_columns in [
        "",
        ",name TEXT NOT NULL DEFAULT ''",
        ",title TEXT NOT NULL DEFAULT ''",
    ] {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../../../data/migrations/0001_initial.sql"))
            .unwrap();
        db.execute_batch(&format!("CREATE TABLE aramhex_heroes (champion_id TEXT PRIMARY KEY,source_id TEXT UNIQUE NOT NULL,win_rate REAL NOT NULL,games INTEGER NOT NULL{existing_columns});
            INSERT INTO champions (champion_id,name) VALUES ('Pantheon','不屈之枪');
            INSERT INTO aramhex_heroes (champion_id,source_id,win_rate,games) VALUES ('Pantheon','80',0.7,12345);
            INSERT INTO app_settings (key,value) VALUES ('aramhex_manifest','keep-newer-manifest');")).unwrap();
        migrate(&db).unwrap();
        let identity =
            crate::db::resolve_champion_identity(&db, "unknown-alias", Some("潘森")).unwrap();
        assert_eq!(identity, Some(("Pantheon".into(), "不屈之枪".into())));
        let cached: (f64, u64, String, String) = db
            .query_row(
                "SELECT win_rate,games,name,title FROM aramhex_heroes WHERE source_id='80'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(cached, (0.7, 12345, "潘森".into(), "不屈之枪".into()));
        migrate(&db).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT value FROM app_settings WHERE key='aramhex_manifest'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "keep-newer-manifest"
        );
    }
}

#[test]
#[ignore = "requires public network"]
fn live_manifest_connection() {
    tauri::async_runtime::block_on(async {
        let clients = DownloadClients::new().unwrap();
        let response = clients
            .fetch("main", "latest.json", 128 * 1024, |bytes| {
                let manifest: Manifest =
                    serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                manifest.validate()
            })
            .await;
        assert!(response.is_ok(), "{response:?}");
        let manifest_bytes = response.unwrap();
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes).unwrap();
        let mut contents = Vec::new();
        for name in FILES {
            contents.push(
                clients
                    .fetch(
                        &format!("data-v{}", manifest.seq),
                        &format!("{}/{name}", manifest.path),
                        manifest.files[name].size,
                        |bytes| manifest.verify(name, bytes),
                    )
                    .await
                    .unwrap(),
            );
        }
        let data = Dataset::parse(
            &manifest_bytes,
            [&contents[0], &contents[1], &contents[2], &contents[3]],
        )
        .unwrap();
        let mut db = database();
        data.import(&mut db).unwrap();
        eprintln!(
            "Live AramHex snapshot verified and imported: seq={} patch={} heroes={}",
            manifest.seq,
            manifest.patch,
            data.heroes.len()
        );
    });
}

#[test]
fn official_data_mirror_is_the_first_direct_route() {
    let routes = download_routes("main", "latest.json");
    assert_eq!(routes[0].url, "https://aramhex.com/u/data/latest.json");
    assert!(routes[0].direct);
    let files = download_routes("data-v12", "patches/16.21/normal/heroes.json");
    assert_eq!(
        files[0].url,
        "https://aramhex.com/u/data/patches/16.21/normal/heroes.json"
    );
    assert!(files
        .iter()
        .any(|r| r.url.contains("data-v12/patches/16.21/normal/heroes.json")));
}

#[test]
fn download_failover_skips_unreachable_and_corrupted_sources() {
    tauri::async_runtime::block_on(async {
        let paths = vec!["offline".to_string(), "corrupt".into(), "healthy".into()];
        let result = first_verified(
            &paths,
            |url| {
                std::future::ready(match url.as_str() {
                    "offline" => Err("连接超时".into()),
                    "corrupt" => Ok(b"bad".to_vec()),
                    _ => Ok(b"valid".to_vec()),
                })
            },
            |bytes| {
                if bytes == b"valid" {
                    Ok(())
                } else {
                    Err("校验失败".into())
                }
            },
        )
        .await;
        assert_eq!(result.unwrap(), b"valid");
        let errors = first_verified(
            &paths[..2],
            |url| std::future::ready(Err(format!("{url}: timeout"))),
            |_| Ok(()),
        )
        .await
        .unwrap_err();
        assert!(errors.contains("offline"));
        assert!(errors.contains("corrupt"));
    });
}

#[test]
fn smoothing_and_sample_warnings() {
    let strong = rate(6000, 10000, 0.5, false);
    assert_eq!(strong.grade.as_deref(), Some("S"));
    assert!(strong.delta_pp.unwrap() > 9.0);
    assert!(strong.reason.contains("优先考虑"));
    let lucky = rate(1, 1, 0.5, true);
    assert_eq!(lucky.grade.as_deref(), Some("A"));
    assert!(lucky.low_sample);
    assert!(lucky.reason.contains("仅供参考"));
    assert_eq!(rate(4000, 10000, 0.5, false).grade.as_deref(), Some("B"));
    assert_eq!(rate(0, 0, 0.5, false).grade, None);
}

fn fixture() -> Dataset {
    Dataset::parse(
        include_bytes!("../../../../data/seed/aramhex/latest.json"),
        [
            include_bytes!("../../../../data/seed/aramhex/heroes.json").as_slice(),
            include_bytes!("../../../../data/seed/aramhex/hero_augments.json").as_slice(),
            include_bytes!("../../../../data/seed/aramhex/augments.json").as_slice(),
            include_bytes!("../../../../data/seed/aramhex/meta.json").as_slice(),
        ],
    )
    .expect("bundled public dataset must verify")
}

fn database() -> rusqlite::Connection {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!("../../../data/migrations/0001_initial.sql"))
        .unwrap();
    migrate(&db).unwrap();
    db.execute(
        "INSERT INTO champions (champion_id,name) VALUES ('Pantheon','潘森')",
        [],
    )
    .unwrap();
    db
}

#[test]
fn verified_dataset_imports_rankings_and_preserves_canonical_ids() {
    let data = fixture();
    let mut db = database();
    data.import(&mut db).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM champion_rankings", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        173
    );
    assert_eq!(
        db.query_row(
            "SELECT source_id FROM aramhex_heroes WHERE champion_id='Pantheon'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "80"
    );
    let rows = crate::db::load_augment_advice(&db, "Pantheon").unwrap();
    assert!(rows
        .iter()
        .any(|r| r.games.is_some_and(|n| n > 0) && r.win_rate.is_some()));
    let excluded = rows.iter().find(|r| r.augment_id == "1337").unwrap();
    assert!(excluded.grade.is_none());
    assert!(excluded.reason.contains("统计失真"));
    assert!(rows.iter().all(|r| r.augment_id != "3176"));
    let unknown = crate::db::load_augment_advice(&db, "not-a-hero").unwrap();
    assert!(unknown.is_empty());
    assert!(!data.needs_update(&db).unwrap());
    let mut next = fixture();
    next.manifest.seq += 1;
    assert!(next.needs_update(&db).unwrap());
    next.manifest.seq = 1;
    assert!(!next.needs_update(&db).unwrap());
}

#[test]
fn corrupted_download_is_rejected() {
    let mut bytes = include_bytes!("../../../../data/seed/aramhex/heroes.json").to_vec();
    bytes[0] = b'!';
    let manifest: Manifest =
        serde_json::from_slice(include_bytes!("../../../../data/seed/aramhex/latest.json"))
            .unwrap();
    assert!(manifest.verify("heroes.json", &bytes).is_err());
}

#[test]
fn invalid_data_cannot_replace_existing_cache() {
    let mut db = database();
    let mut data = fixture();
    data.import(&mut db).unwrap();
    data.heroes.get_mut("80").unwrap().games = 0;
    assert!(data.import(&mut db).is_err());
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM champion_rankings", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        173
    );
}

#[test]
fn invalid_manifest_paths_and_missing_files_are_rejected() {
    let mut data = fixture();
    data.manifest.path = "../secret".into();
    assert!(data.manifest.validate().is_err());
    data = fixture();
    data.manifest.files.remove("hero_augments.json");
    assert!(data.manifest.validate().is_err());
}

#[test]
fn legacy_seed_all_heroes_keep_their_english_ids_and_migration_is_repeatable() {
    let mut db = database();
    crate::db::snapshot::import_augment_grades_json(
        &mut db,
        include_str!("../../../../data/seed/augment_grades.json"),
    )
    .unwrap();
    migrate(&db).unwrap();
    fixture().import(&mut db).unwrap();
    let numeric: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM aramhex_heroes WHERE champion_id=source_id",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(numeric, 0);
    assert!(!crate::db::load_augment_advice(&db, "80")
        .unwrap()
        .is_empty());
    assert!(!crate::db::load_augment_advice(&db, "pantheon")
        .unwrap()
        .is_empty());
}

#[test]
fn transaction_write_failure_restores_every_table_and_manifest() {
    let mut db = database();
    let mut data = fixture();
    data.import(&mut db).unwrap();
    let old: String = db
        .query_row(
            "SELECT value FROM app_settings WHERE key='aramhex_manifest'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    db.execute_batch("CREATE TRIGGER reject_new_statistics BEFORE INSERT ON aramhex_augment_stats BEGIN SELECT RAISE(ABORT,'injected write failure'); END;").unwrap();
    data.manifest.seq += 1;
    assert!(data.import(&mut db).is_err());
    let current: String = db
        .query_row(
            "SELECT value FROM app_settings WHERE key='aramhex_manifest'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(current, old);
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM champion_rankings", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        173
    );
    assert!(!crate::db::load_augment_advice(&db, "Pantheon")
        .unwrap()
        .is_empty());
}

#[test]
fn future_champion_is_resolvable_by_client_chinese_name() {
    let mut db = database();
    fixture().import(&mut db).unwrap();
    let identity = crate::db::resolve_champion_identity(&db, "Annie", Some("安妮"))
        .unwrap()
        .unwrap();
    assert_eq!(identity.0, "1");
    assert!(!crate::db::load_augment_advice(&db, &identity.0)
        .unwrap()
        .is_empty());
}

#[test]
fn augment_exposure_cannot_exceed_hero_participations() {
    let mut data = fixture();
    let h = &data.heroes["80"];
    let games = h.games + 10;
    let wins = h.wins;
    let s = data
        .hero_augments
        .get_mut("80")
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    s.games = games;
    s.wins = wins;
    s.win_rate = wins as f64 / games as f64;
    assert!(data.validate().is_err());
}
