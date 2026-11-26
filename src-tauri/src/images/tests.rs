use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

#[test]
fn relative_fixed_and_grouped_images_remain_readable_without_overwrite() {
    let base = std::env::temp_dir().join(format!("jot-images-{}", uuid::Uuid::new_v4()));
    let docs = base.join("docs");
    let fixed = base.join("shared");
    fs::create_dir_all(&docs).unwrap();
    fs::create_dir(&fixed).unwrap();
    let doc = docs.join("会议纪要.md");
    let data = b"GIF89a fixture";
    let cfg = Config {
        group_by_document: true,
        ..Config::default()
    };
    let first = local::save(&doc, &cfg, Destination::Relative, "图 1", "gif", data).unwrap();
    assert!(first.starts_with("_assets/") && !first.contains(' '));
    let second = local::save(
        &doc,
        &cfg,
        Destination::Relative,
        "图 1",
        "gif",
        b"GIF89a second",
    )
    .unwrap();
    assert_ne!(first, second);
    assert_eq!(local::read(&doc, &first, &[]).unwrap().1, data);
    let fixed = fixed.canonicalize().unwrap();
    let cfg = Config {
        default_mode: Destination::Fixed,
        directory: fixed.clone(),
        ..cfg
    };
    let link = local::save(&doc, &cfg, Destination::Fixed, "图 1", "gif", data).unwrap();
    assert!(link.starts_with("file:///"));
    assert!(local::read(&doc, &link, &[]).is_none());
    assert_eq!(
        local::read(&doc, &link, std::slice::from_ref(&fixed))
            .unwrap()
            .1,
        data
    );
    assert!(local::validate_folder("../outside").is_err());
    assert!(local::read(&doc, "%2e%2e/shared/image.gif", &[]).is_none());
    fs::write(docs.join("fake.png"), b"not an image").unwrap();
    assert!(local::read(&doc, "fake.png", &[]).is_none());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&fixed, docs.join("link")).unwrap();
        assert!(
            local::save(
                &doc,
                &Config {
                    subfolder: "link/nested".into(),
                    ..Config::default()
                },
                Destination::Relative,
                "a",
                "gif",
                data
            )
            .is_err()
        );
        assert!(!fixed.join("nested").exists());
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
#[ignore = "uses a loopback HTTP listener"]
fn multipart_upload_sends_image_and_validates_returned_destination() {
    for (status, body, success) in [
        (
            "200 OK",
            r#"{"data":{"image":"https://images.example.test/a.png"}}"#,
            true,
        ),
        (
            "200 OK",
            r#"{"data":{"image":"http://images.example.test/a.png"}}"#,
            true,
        ),
        (
            "200 OK",
            r#"{"data":{"image":"file:///tmp/photo.png"}}"#,
            false,
        ),
        ("200 OK", r#"{"data":{}}"#, false),
        ("401 Unauthorized", "{}", false),
    ] {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = Config {
            endpoint: format!("http://{}/upload", server.local_addr().unwrap()),
            field: "photo".into(),
            response_pointer: "/data/image".into(),
            ..Config::default()
        };
        let handle = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            let mut data = Vec::new();
            let mut buf = [0; 2048];
            loop {
                let count = client.read(&mut buf).unwrap();
                assert!(count > 0);
                data.extend_from_slice(&buf[..count]);
                if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&data[..i]).to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if data.len() >= i + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8(data).unwrap();
            assert!(text.starts_with("POST /upload "));
            assert!(
                text.to_ascii_lowercase()
                    .contains("authorization: bearer fixture")
            );
            assert!(
                text.contains("name=\"photo\"")
                    && text.contains("filename=\"a.gif\"")
                    && text.contains("GIF89a fixture")
            );
            write!(
                client,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let result = upload::send(
            &config,
            "a.gif",
            "image/gif",
            b"GIF89a fixture".to_vec(),
            Some("fixture"),
        );
        handle.join().unwrap();
        assert_eq!(result.is_ok(), success);
        if success {
            let json: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(result.unwrap(), json["data"]["image"].as_str().unwrap());
        }
    }
}

#[test]
fn folder_specific_image_paths_and_upload_protocols() {
    let fixture = std::env::temp_dir().join(format!("jot-blog-{}", uuid::Uuid::new_v4()));
    let posts = fixture.join("blog/posts");
    let private = posts.join("private");
    fs::create_dir_all(&private).unwrap();
    let base = fixture.canonicalize().unwrap();
    let config_json = serde_json::json!({
        "endpoint": "http://uploads.example.test/image",
        "rules": [
            {"path": base.join("blog"), "mode": "fixed", "directory": "${ruleDir}/../access"},
            {"path": private, "mode": "relative", "subfolder": "media/screenshots"}
        ]
    });
    let mut config: Config = serde_json::from_value(config_json).unwrap();
    config.validate(&Config::default()).unwrap();
    assert!(crate::settings::filing::endpoint(&config.endpoint).is_err());
    let data = b"GIF89a fixture";
    for (doc, expected, saved) in [
        ("blog/posts/public.md", "../../access/a.gif", "access/a.gif"),
        (
            "blog/posts/private/n.md",
            "media/screenshots/a.gif",
            "blog/posts/private/media/screenshots/a.gif",
        ),
        ("blog-other.md", "_assets/a.gif", "_assets/a.gif"),
    ] {
        let doc = base.join(doc);
        let link = local::save(&doc, &config, config.mode_for(&doc), "a", "gif", data).unwrap();
        assert_eq!(link, expected);
        assert_eq!(fs::read(base.join(saved)).unwrap(), data);
        if link.starts_with("..") {
            assert!(local::read(&doc, &link, &[]).is_none());
        }
        assert_eq!(
            local::read(&doc, &link, &[base.join("access")]).unwrap().1,
            data
        );
    }
    let mut invalid = config.clone();
    invalid.rules[1].subfolder = "../private".into();
    assert!(invalid.validate(&config).is_err());
    invalid = config.clone();
    invalid.rules[0].directory = "unanchored".into();
    assert!(invalid.validate(&config).is_err());
    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn folder_rules_persist_match_components_and_inherit_the_nearest_parent() {
    use super::config::FolderRule;
    let base = std::env::temp_dir().join(format!("jot-image-rules-{}", uuid::Uuid::new_v4()));
    let workspace = base.join("work");
    let nested = workspace.join("docs");
    let archive = base.join("archive");
    let shared = base.join("assets");
    for folder in [&nested, &archive, &shared] {
        fs::create_dir_all(folder).unwrap();
    }
    let make_rule = |path: &Path, mode| FolderRule {
        path: path.to_owned(),
        mode,
        directory: PathBuf::new(),
        subfolder: String::new(),
    };
    let mut config = Config {
        default_mode: Destination::Relative,
        endpoint: "https://example.test/upload".into(),
        directory: shared.clone(),
        rules: vec![
            make_rule(&nested, Destination::Http),
            make_rule(&archive, Destination::Fixed),
            make_rule(&workspace, Destination::Fixed),
        ],
        ..Config::default()
    };
    config.validate(&Config::default()).unwrap();
    let file = base.join("images.json");
    storage::write_json(
        &file,
        &Saved {
            config,
            directories: vec![shared],
        },
    )
    .unwrap();
    let mut config = Images::load(file.clone()).unwrap().saved.config;
    let base = base.canonicalize().unwrap();
    for (file, expected) in [
        ("work/n.md", Destination::Fixed),
        ("work/docs/deep/n.md", Destination::Http),
        ("archive/meeting.md", Destination::Fixed),
        ("work-other/n.md", Destination::Relative),
    ] {
        assert_eq!(config.mode_for(&base.join(file)), expected);
    }
    config.rules.remove(0);
    assert_eq!(
        config.mode_for(&base.join("work/docs/deep/n.md")),
        Destination::Fixed
    );
    let mut duplicate = config.clone();
    duplicate.rules.push(config.rules[0].clone());
    assert!(duplicate.validate(&config).is_err());
    let mut incomplete = config.clone();
    incomplete.directory = PathBuf::new();
    assert!(incomplete.validate(&config).is_err());
    incomplete = config.clone();
    incomplete.rules.push(make_rule(&base, Destination::Http));
    incomplete.endpoint.clear();
    assert!(incomplete.validate(&config).is_err());

    fs::write(
        &file,
        br#"{"config":{"mode":"fixed","groupByDocument":true},"directories":[]}"#,
    )
    .unwrap();
    let previous = Images::load(file).unwrap().saved.config;
    assert_eq!(previous.default_mode, Destination::Fixed);
    assert!(previous.group_by_document && previous.rules.is_empty());
    fs::remove_dir_all(base).unwrap();
}
