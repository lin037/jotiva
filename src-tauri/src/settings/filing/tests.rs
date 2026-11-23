use super::*;

mod live;

#[test]
fn old_or_unusable_preferences_never_enable_a_service() {
    let file =
        std::env::temp_dir().join(format!("jot-disabled-filing-{}.json", std::process::id()));
    for json in [
        "true",
        "false",
        "{",
        r#"{"enabled":true,"endpoint":"http://insecure.invalid"}"#,
    ] {
        fs::write(&file, json).unwrap();
        let mut filing = Filing::load(file.clone()).unwrap();
        assert!(filing.begin(1, 1).is_none());
        assert_eq!(fs::read_to_string(&file).unwrap(), json);
    }
    for url in [
        "file:///tmp",
        "http://localhost:8080",
        "http://127.1",
        "http://127.0.0.2",
        "http://192.168.3.9",
        "http://127.0.0.1@host.invalid",
        "http://[::1]@host.invalid",
        "http://[::ffff:127.0.0.1]",
        "http://example.test/path",
        "https://key@example.test",
        "https://example.test?key=secret",
        "https://example.test#key",
    ] {
        assert!(endpoint(url).is_err());
    }
    let url = "https://example.test/custom/path";
    write_json(&file, &json!({"enabled":true,"endpoint":url})).unwrap();
    let mut filing = Filing::load(file.clone()).unwrap();
    assert_eq!(filing.config.service_url().unwrap(), url);
    assert_eq!(filing.config.model, DEFAULT_MODEL);
    assert!(filing.begin(1, 1).is_some());
    write_json(
        &file,
        &json!({"enabled":false, "endpoint":"", "model":"local-choice", "addressKind":"base"}),
    )
    .unwrap();
    let filing = Filing::load(file.clone()).unwrap();
    assert!(!filing.config.enabled);
    assert_eq!(filing.config.model, "local-choice");
    assert!(matches!(filing.config.address_kind, AddressKind::Base));
    fs::remove_file(file).unwrap();
}

#[test]
fn base_addresses_keep_the_gateway_prefix_and_full_endpoints_remain_exact() {
    for (address, suffix) in [
        ("https://gateway.invalid", "/v1/systemone"),
        ("https://gateway.invalid/", "/v1/systemone"),
        (
            "https://gateway.invalid:9443/custom%20path/",
            "/custom%20path/v1/systemone",
        ),
        ("https://gateway.invalid/v1", "/v1/systemone"),
        ("https://gateway.invalid/custom/v1/", "/custom/v1/systemone"),
        ("https://v1", "/v1/systemone"),
        ("http://127.0.0.1:8080/gateway/", "/gateway/v1/systemone"),
        ("http://[::1]:8080/v1/", "/v1/systemone"),
    ] {
        let config = Config {
            endpoint: address.into(),
            ..Config::default()
        };
        let got = config.service_url().unwrap();
        let got = tauri::Url::parse(&got).unwrap();
        assert_eq!(got.path(), suffix);
        assert_eq!(got.origin(), endpoint(address).unwrap().origin());
    }
    assert!(endpoint("https://example.test/split\npath").is_err());
    assert!(endpoint(r"https://example.test\route").is_err());
    assert_eq!(model_name("  local-choice ").unwrap(), "local-choice");
    assert_eq!(model_name(" ").unwrap(), DEFAULT_MODEL);
    assert!(model_name("local\nname").is_err());
}

fn folder(path: PathBuf) -> Folder {
    Folder {
        id: 7,
        name: "研发".into(),
        classify: true,
        description: "项目设计与代码评审".into(),
        path,
    }
}

#[test]
fn choice_contract_contains_only_note_and_descriptions() {
    let mut second = folder("/never-send/second".into());
    second.name = "会议".into();
    second.id = 8;
    let folders = [folder("/never-send/private-folder".into()), second];
    let body = request(&"测".repeat(7000), &folders, DEFAULT_MODEL).unwrap();
    assert_eq!(body["state"].as_str().unwrap().chars().count(), 6000);
    assert_eq!(body["model"], "jev-latest");
    let item = &body["questions"]["destination"];
    assert_eq!(item["type"], "choice");
    assert_eq!(item["criteria"]["folder_7"], "研发：项目设计与代码评审");
    assert!(item["criteria"]["other"].is_string());
    assert_eq!(item["criteria"].as_object().unwrap().len(), 3);
    assert!(!body.to_string().contains("never-send"));
    for (choice, expected) in [
        ("folder_7", Some(7)),
        ("folder_8", Some(8)),
        ("other", None),
    ] {
        let reply = json!({"answers":{"destination":{"type":"choice","choice":choice}}});
        assert_eq!(
            destination(reply.to_string().as_bytes(), &folders).unwrap(),
            expected
        );
    }
    assert!(
        destination(
            br#"{"answers":{"destination":{"type":"choice","choice":"/tmp/elsewhere"}}}"#,
            &folders
        )
        .is_err()
    );
    for kind in ["score", "noul", "box"] {
        let reply = json!({"answers":{"destination":{"type":kind,"choice":"folder_7"}}});
        assert!(destination(reply.to_string().as_bytes(), &folders).is_err());
    }
    assert!(destination(b"{", &folders).is_err());
    assert!(request("文档", &vec![folders[0].clone(); 255], DEFAULT_MODEL).is_err());
    assert!(
        request(
            &format!("{}！ ## 🌈 ", "文".repeat(19)),
            &folders,
            DEFAULT_MODEL
        )
        .is_err()
    );
    assert!(
        request(
            &format!("{}{}", "#".repeat(6000), "文".repeat(20)),
            &folders,
            DEFAULT_MODEL
        )
        .is_err()
    );
    assert!(request(&"文".repeat(20), &folders, DEFAULT_MODEL).is_ok());
}

#[test]
fn one_attempt_per_saved_revision_and_manual_override_wins() {
    let mut state = Filing {
        config: Config::default(),
        file: PathBuf::new(),
        sequence: 0,
        attempts: HashMap::new(),
    };
    assert!(state.begin(8, 1).is_none());
    state.config.enabled = true;
    let first = state.begin(8, 1).unwrap();
    assert!(state.begin(8, 1).is_none());
    let newer = state.begin(8, 2).unwrap();
    state.cancel(8, 1, first);
    assert_eq!(state.pending_for_edit(8, 2), Some(newer));
    state.manually_moved(8);
    assert_eq!(state.pending_for_edit(8, 2), None);
    // If a WebView closed without responding to a suggestion, a later manual request can replace it.
    let abandoned = state.begin(8, 2).unwrap();
    state.attempts.get_mut(&8).unwrap().suggestion = Some(folder(PathBuf::new()));
    let retry = state.begin(8, 2).unwrap();
    state.cancel(8, 2, abandoned);
    state.cancel(8, 2, newer);
    assert_eq!(state.pending_for_edit(8, 2), Some(retry));
    state.cancel(8, 2, retry);
    assert!(state.attempts.is_empty());
}

#[test]
fn retries_transient_failure_once_and_respects_cancellation() {
    for code in [-2, 0, 502, 503, 504] {
        let mut calls = 0;
        let run = send_request(
            || {
                calls += 1;
                Ok((if calls == 1 { code } else { 200 }, b"result".to_vec()))
            },
            || {},
        );
        assert_eq!(calls, 2);
        assert_eq!(run.unwrap(), b"result");
    }
    for code in [-1, -3, -4, 302, 400, 401, 403, 429] {
        assert!(
            send_request(
                || Ok((code, Vec::new())),
                || panic!("permanent failure retried")
            )
            .is_err()
        );
    }
    let mut calls = 0;
    assert!(
        send_request(
            || {
                calls += 1;
                Ok((503, Vec::new()))
            },
            || {}
        )
        .is_err()
    );
    assert_eq!(calls, 2);
    let active = std::cell::Cell::new(true);
    let mut sent = 0;
    assert!(
        send_request(
            || {
                if !active.get() {
                    return Err("cancelled".into());
                }
                sent += 1;
                Ok((503, Vec::new()))
            },
            || active.set(false)
        )
        .is_err()
    );
    assert_eq!(sent, 1);
}

#[test]
fn only_the_current_unfiled_note_creates_a_markdown_file() {
    let base = std::env::temp_dir().join(format!("jot-filing-{}", std::process::id()));
    let dest = base.join("managed");
    fs::create_dir_all(&dest).unwrap();
    let dest = folder(fs::canonicalize(dest).unwrap());
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let id = notes.new_note(None, "private source window".into());
    assert!(notes.filing_text(id, 0).is_none());
    notes.save(id, 0, " \n ".into()).unwrap();
    assert!(notes.filing_text(id, 1).is_none());
    notes.save(id, 1, "# 设计".into()).unwrap();
    assert!(apply_destination(&mut notes, id, 1, &dest, &dest).is_err());
    let mut changed = dest.clone();
    changed.classify = false;
    assert!(apply_destination(&mut notes, id, 2, &dest, &changed).is_err());
    let source = notes.disk_path(id).unwrap().to_owned();
    fs::write(&source, "外部编辑").unwrap();
    assert!(verify_saved_note(&notes, id, 2).is_err());
    assert!(apply_destination(&mut notes, id, 2, &dest, &dest).is_err());
    assert_eq!(fs::read_to_string(&source).unwrap(), "外部编辑");
    fs::write(&source, "# 设计").unwrap();
    // A matching filename belongs to its creator and must never be overwritten.
    let existing = dest.path.join("设计.md");
    fs::write(&existing, "原有文件").unwrap();
    assert!(apply_destination(&mut notes, id, 2, &dest, &dest).is_ok());
    assert_eq!(fs::read_to_string(&existing).unwrap(), "原有文件");
    let assigned = dest.path.join("设计 (2).md");
    assert_eq!(
        notes.note(id).unwrap().disk_path(),
        Some(assigned.as_path())
    );
    assert_eq!(fs::read_to_string(&assigned).unwrap(), "# 设计");
    assert!(notes.filing_text(id, 2).is_none());
    assert!(apply_destination(&mut notes, id, 2, &dest, &dest).is_err());
    let external = notes.open_file(&existing).unwrap();
    assert!(notes.filing_text(external, 1).is_none());
    assert!(apply_destination(&mut notes, external, 1, &dest, &dest).is_err());
    fs::remove_dir_all(base).unwrap();
}

use crate::platform::filing::post_fixture;

#[test]
#[ignore = "requires a local HTTP listener"]
fn loopback_choice_requires_confirmation_before_writing() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    const TEXT: &str = "# 周会记录\n本次会议集中讨论新版本的研发设计与代码评审计划。";
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let gateway = Config {
        endpoint: format!("http://{}/gateway", listener.local_addr().unwrap()),
        model: "local-choice".into(),
        ..Config::default()
    };
    let url = gateway.service_url().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "POST /gateway/v1/systemone HTTP/1.1\r\n");
        let mut headers = HashMap::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.trim_end().split_once(':').unwrap();
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
        assert_eq!(headers["authorization"], "Bearer local-fixture");
        assert_eq!(headers["content-type"], "application/json");
        let mut body = vec![0; headers["content-length"].parse().unwrap()];
        reader.read_exact(&mut body).unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 3);
        assert_eq!(json["model"], "local-choice");
        assert_eq!(json["state"], TEXT);
        let questions = json["questions"].as_object().unwrap();
        assert_eq!(questions.len(), 1);
        let decision = &questions["destination"];
        assert_eq!(decision["type"], "choice");
        assert!(decision["instructions"].as_str().unwrap().contains("other"));
        assert_eq!(
            decision["criteria"],
            json!({"folder_7":"研发：项目设计与代码评审","other":"其他：没有合适的目录，或信息不足"})
        );
        let answer = br#"{"answers":{"destination":{"type":"choice","choice":"folder_7"}}}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            answer.len()
        )
        .unwrap();
        stream.write_all(answer).unwrap();
    });

    let base = std::env::temp_dir().join(format!("jot-loopback-filing-{}", std::process::id()));
    let archive = base.join("archive");
    fs::create_dir_all(&archive).unwrap();
    let archive = archive.canonicalize().unwrap();
    let config = base.join("folders.json");
    fs::create_dir(base.join("private")).unwrap();
    let mut private = folder(base.join("private").canonicalize().unwrap());
    private.id = 9;
    private.name = "excluded".into();
    private.classify = false;
    write_json(&config, &[folder(archive.clone()), private]).unwrap();
    let folders = Folders::load_fixture(config).unwrap();
    let candidates = folders.filing_candidates();
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let id = notes.new_note(None, "local source window".into());
    let version = notes.save(id, 0, TEXT.into()).unwrap();
    let source = notes.disk_path(id).unwrap().to_owned();
    let mut filing = Filing::load(base.join("filing.json")).unwrap();
    filing.config.enabled = true;
    let ticket = filing.begin(id, version).unwrap();
    verify_saved_note(&notes, id, version).unwrap();
    let payload = request(
        notes.filing_text(id, version).unwrap(),
        &candidates,
        &gateway.model,
    )
    .unwrap();
    let result = query_model(&payload, &candidates, |payload| {
        Ok(post_fixture(&url, "local-fixture", payload))
    });
    server.join().unwrap();
    let proposal = filing.propose(id, version, ticket, result).unwrap();
    assert!(
        matches!(proposal, Some(FilingReply::Suggestion { folder, ticket: expected }) if folder == "研发" && expected == ticket)
    );
    assert_eq!(fs::read_dir(&archive).unwrap().count(), 0);
    assert_eq!(fs::read_to_string(&source).unwrap(), TEXT);
    assert!(filing.take_suggestion(id, version, ticket + 1).is_err());
    let chosen = filing.take_suggestion(id, version, ticket).unwrap();
    apply_destination(
        &mut notes,
        id,
        version,
        &chosen,
        folders.destination(chosen.id).unwrap(),
    )
    .unwrap();
    assert!(filing.take_suggestion(id, version, ticket).is_err());
    assert!(!source.exists());
    let moved = notes.disk_path(id).unwrap();
    assert_eq!(moved.parent().unwrap(), archive);
    assert_eq!(fs::read_to_string(moved).unwrap(), TEXT);
    fs::remove_dir_all(base).unwrap();
}
