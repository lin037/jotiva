use super::*;
use crate::library::folders::FolderInput;
use crate::storage::write_json;

fn temporary(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("jot-files-{}-{name}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    directory.canonicalize().unwrap()
}

#[test]
fn naming_empty_capture_publishes_the_same_note_without_overwriting_files() {
    let base = temporary("rename-capture");
    let mut workspace = Workspace::load(base.join("workspace.json")).unwrap();
    let mut folders = Folders::load_fixture(base.join("folders.json")).unwrap();
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let sibling = notes.create_window_note(Some(42), "会议".into()).unwrap();
    let id = notes.new_note(Some(42), "会议".into());
    let mut rename = |name: &str| {
        manage_note(
            &mut workspace,
            &mut folders,
            &mut notes,
            id,
            NoteAction::Rename { name: name.into() },
        )
    };
    assert!(rename("新便签.md").is_err());
    assert_eq!(rename("行动清单.md").unwrap().0, id);
    let path = notes.card(id).unwrap().archive_path.unwrap();
    assert_eq!(path.file_name().unwrap(), "行动清单.md");
    notes.save(id, 1, "# 周会".into()).unwrap();
    assert_eq!(notes.host(id), Some(42));
    assert_eq!(fs::read_to_string(path).unwrap(), "# 周会");
    assert_eq!(notes.note(sibling).unwrap().markdown, "");
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn reserved_inbox_rename_and_duplicate_keep_physical_archive_membership() {
    let base = temporary("inbox-actions");
    let mut workspace = Workspace::load(base.join("workspace.json")).unwrap();
    let mut folders = Folders::load_fixture(base.join("folders.json")).unwrap();
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let id = notes.new_note(None, "Jotiva".into());
    notes.save(id, 0, "# 纪要".into()).unwrap();
    let original = notes.note(id).unwrap().disk_path().unwrap().to_owned();
    assert_eq!(fs::read_to_string(&original).unwrap(), "# 纪要");
    manage_note(
        &mut workspace,
        &mut folders,
        &mut notes,
        id,
        NoteAction::Rename {
            name: "会议纪要".into(),
        },
    )
    .unwrap();
    let card = notes.card(id).unwrap();
    let renamed = card.archive_path.unwrap();
    assert_eq!(renamed, notes.inbox_directory().join("会议纪要.md"));
    assert!(card.file.is_none() && card.folder.is_none() && !original.exists());
    let (copy, _) = manage_note(
        &mut workspace,
        &mut folders,
        &mut notes,
        id,
        NoteAction::Duplicate,
    )
    .unwrap();
    assert_eq!(notes.open_file(&renamed).unwrap(), id);
    let copy_path = notes.card(copy).unwrap().archive_path.unwrap();
    assert_eq!(copy_path.parent(), renamed.parent());
    assert_eq!(fs::read_to_string(copy_path).unwrap(), "# 纪要");
    let from_finder = notes.inbox_directory().join("外部放入.md");
    fs::write(&from_finder, "示例").unwrap();
    let imported = notes.open_file(&from_finder).unwrap();
    assert_eq!(
        notes.card(imported).unwrap().archive_path,
        Some(from_finder)
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn a_finder_document_can_manage_its_file_without_registering_a_workspace() {
    let base = temporary("finder-file");
    let mut workspace = Workspace::load(base.join("workspace.json")).unwrap();
    let mut folders = Folders::load_fixture(base.join("folders.json")).unwrap();
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let path = base.join("会议 记录.markdown");
    fs::write(&path, "# 初稿").unwrap();
    let id = notes.open_file(&path).unwrap();
    let mut act = |action| manage_note(&mut workspace, &mut folders, &mut notes, id, action);
    let (copy, _) = act(NoteAction::Duplicate).unwrap();
    let title = "会议纪要.markdown";
    assert_eq!(
        act(NoteAction::Rename { name: title.into() }).unwrap().0,
        id
    );
    assert!(!path.exists());
    let renamed = base.join(title);
    assert_eq!(notes.note(id).unwrap().disk_path(), Some(renamed.as_path()));
    assert_eq!(fs::read_to_string(&renamed).unwrap(), "# 初稿");
    assert_ne!(copy, id);
    assert_eq!(
        notes.note(copy).unwrap().file.unwrap(),
        base.join("会议 记录 副本.markdown")
    );
    assert!(workspace.roots.is_empty());

    // A directory replaced the opened file outside Jotiva. A file-menu action must never trash its contents.
    fs::remove_file(&renamed).unwrap();
    fs::create_dir(&renamed).unwrap();
    assert!(
        manage_note(
            &mut workspace,
            &mut folders,
            &mut notes,
            id,
            NoteAction::Trash
        )
        .is_err()
    );
    assert!(renamed.is_dir());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn moving_a_subtree_keeps_ids_open_files_and_nested_roots_across_restart() {
    let base = temporary("move");
    let root = base.join("project");
    let group = root.join("meetings");
    fs::create_dir_all(&group).unwrap();
    let config = base.join("workspace.json");
    write_json(&config, &[&root, &group]).unwrap();
    let mut workspace = Workspace::load(config.clone()).unwrap();
    let mut folders = Folders::load_fixture(base.join("folders.json")).unwrap();
    let archive = folders
        .save(
            FolderInput {
                id: None,
                name: "会议".into(),
                classify: true,
                description: String::new(),
                path: group.clone(),
            },
            false,
        )
        .unwrap();
    let notes_dir = base.join("notes");
    let mut notes = Notes::load(notes_dir.clone()).unwrap();
    let archived = notes
        .create_named("记录".into(), "议程".into(), Some(&archive))
        .unwrap();
    let file = group.join("original.md");
    fs::write(&file, "原稿").unwrap();
    let external = notes.open_file(&file).unwrap();
    notes
        .attach_to_host(external, 8, "测试窗口".into())
        .unwrap();

    let target = root.join("minutes");
    relocate(
        &mut workspace,
        &mut folders,
        &mut notes,
        &group,
        Some(target.clone()),
    )
    .unwrap();
    assert_eq!(notes.host(external), Some(8));
    assert_eq!(workspace.roots, [&root, &target].map(PathBuf::clone));
    assert_eq!(folders.get(archive.id).unwrap().path, target);
    assert_eq!(
        notes.note(archived).unwrap().location.unwrap().path,
        target.join("记录.md")
    );
    assert_eq!(
        notes.open_file(&target.join("original.md")).unwrap(),
        external
    );
    notes.save(external, 1, "继续编辑".into()).unwrap();
    assert_eq!(
        fs::read_to_string(target.join("original.md")).unwrap(),
        "继续编辑"
    );
    assert!(!file.exists());
    let mut notes = Notes::load(notes_dir).unwrap();
    assert_eq!(
        notes.open_file(&target.join("original.md")).unwrap(),
        external
    );
    assert_eq!(Workspace::load(config).unwrap().roots, workspace.roots);
    assert_eq!(
        Folders::load_fixture(base.join("folders.json"))
            .unwrap()
            .get(archive.id)
            .unwrap()
            .path,
        target
    );
    // Move the archive document outside its registered destination. It remains an editable file with its original id.
    let moved = root.join("会议纪要.md");
    relocate(
        &mut workspace,
        &mut folders,
        &mut notes,
        &target.join("记录.md"),
        Some(moved.clone()),
    )
    .unwrap();
    assert_eq!(notes.note(archived).unwrap().file, Some(moved));
    assert!(notes.note(archived).unwrap().location.is_none());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn conflict_and_metadata_write_failure_restore_the_original_tree() {
    let base = temporary("rollback");
    let root = base.join("work");
    let occupied = base.join("已有");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&occupied).unwrap();
    let config = base.join("workspace.json");
    write_json(&config, &[&root]).unwrap();
    let mut workspace = Workspace::load(config.clone()).unwrap();
    let mut folders = Folders::load_fixture(base.join("folders.json")).unwrap();
    let mut notes = Notes::load(base.join("notes")).unwrap();
    let file = root.join("a.md");
    fs::write(&file, "正文").unwrap();
    let id = notes.open_file(&file).unwrap();
    assert!(
        relocate(
            &mut workspace,
            &mut folders,
            &mut notes,
            &root,
            Some(occupied.clone())
        )
        .is_err()
    );
    assert!(!occupied.join("a.md").exists());
    let target = base.join("new");
    let valid_config = config.with_extension("saved");
    fs::rename(&config, &valid_config).unwrap();
    fs::create_dir(&config).unwrap();
    assert!(
        relocate(
            &mut workspace,
            &mut folders,
            &mut notes,
            &root,
            Some(target.clone())
        )
        .is_err()
    );
    assert!(file.is_file() && !target.exists());
    assert_eq!(notes.note(id).unwrap().file, Some(file));
    fs::remove_dir(&config).unwrap();
    fs::rename(valid_config, &config).unwrap();
    assert_eq!(Workspace::load(config).unwrap().roots, vec![root]);
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn directory_creation_copy_deletion_and_archive_names_use_real_files() {
    let base = temporary("actions");
    let root = base.join("work");
    fs::create_dir(&root).unwrap();
    let config = base.join("workspace.json");
    write_json(&config, &[&root]).unwrap();
    let workspace = Mutex::new(Workspace::load(config).unwrap());
    let folders = Mutex::new(Folders::load_fixture(base.join("folders.json")).unwrap());
    let notes = RwLock::new(Notes::load(base.join("notes")).unwrap());
    let sub = perform_workspace(
        workspace.lock().unwrap(),
        &folders,
        &notes,
        &root,
        FileAction::CreateDirectory {
            name: "小组".into(),
        },
    )
    .unwrap()
    .path
    .unwrap();
    let file = perform_workspace(
        workspace.lock().unwrap(),
        &folders,
        &notes,
        &sub,
        FileAction::CreateFile {
            name: Some("文档".into()),
        },
    )
    .unwrap()
    .path
    .unwrap();
    assert_eq!(file, sub.join("文档.md"));
    assert!(
        perform_workspace(
            workspace.lock().unwrap(),
            &folders,
            &notes,
            &sub,
            FileAction::CreateFile {
                name: Some("文档".into())
            }
        )
        .is_err()
    );
    fs::write(&file, "真实原文件").unwrap();
    fs::write(sub.join("未命名.md"), "已有文档").unwrap();
    for expected in ["未命名 (2).md", "未命名 (3).md"] {
        let created = perform_workspace(
            workspace.lock().unwrap(),
            &folders,
            &notes,
            &sub,
            FileAction::CreateFile { name: None },
        )
        .unwrap()
        .path
        .unwrap();
        assert_eq!(created, sub.join(expected));
        assert_eq!(fs::read_to_string(created).unwrap(), "");
    }
    assert_eq!(
        fs::read_to_string(sub.join("未命名.md")).unwrap(),
        "已有文档"
    );
    let sibling = copy_sibling(&sub).unwrap();
    assert_eq!(
        fs::read_to_string(sibling.join("文档.md")).unwrap(),
        "真实原文件"
    );
    assert!(copy_sibling(&sub).unwrap().ends_with("小组 副本 2"));
    assert!(
        perform_workspace(
            workspace.lock().unwrap(),
            &folders,
            &notes,
            &sub,
            FileAction::Move {
                directory: sub.clone()
            }
        )
        .is_err()
    );
    assert!(markdown_name("../escaped").is_err());
    let mut workspace = workspace.into_inner().unwrap();
    let mut folders = folders.into_inner().unwrap();
    let archive = folders
        .save(
            FolderInput {
                id: None,
                name: "归档".into(),
                classify: true,
                description: String::new(),
                path: sub.clone(),
            },
            false,
        )
        .unwrap();
    let mut notes = notes.into_inner().unwrap();
    let id = notes.new_note(None, "Jotiva".into());
    assert!(notes.disk_path(id).is_none());
    notes.save(id, 0, "#".into()).unwrap();
    let placeholder = notes.disk_path(id).unwrap().to_owned();
    assert_eq!(placeholder, notes.inbox_directory().join("未命名.md"));
    notes.save(id, 1, "# 文档\n正文".into()).unwrap();
    let named = notes.disk_path(id).unwrap().to_owned();
    assert!(!placeholder.exists());
    assert_eq!(named, notes.inbox_directory().join("文档.md"));
    notes.save(id, 2, "# 替换标题".into()).unwrap();
    assert_eq!(notes.disk_path(id), Some(named.as_path()));
    assert_eq!(notes.card(id).unwrap().title, "文档");
    fs::write(sub.join("文档 (2).md"), "同名原件").unwrap();
    let saved = notes.move_to(id, Some(&archive)).unwrap().path.unwrap();
    assert!(saved.ends_with("文档 (3).md"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "真实原文件");
    assert_eq!(
        fs::read_to_string(sub.join("文档 (2).md")).unwrap(),
        "同名原件"
    );
    let copied = notes.duplicate(id, Some(&archive)).unwrap();
    assert_eq!(
        fs::read_to_string(notes.note(copied).unwrap().disk_path().unwrap()).unwrap(),
        "# 替换标题"
    );
    let external = notes.open_file(&file).unwrap();
    // Exercise the same post-trash commit in a temporary directory instead of writing to the user's Trash.
    fs::rename(&sub, base.join("trash")).unwrap();
    let mut result = FileResult::default();
    update_paths(
        &mut workspace,
        &mut folders,
        &mut notes,
        &sub,
        None,
        &mut result,
    )
    .unwrap();
    result.removed.sort();
    let mut expected = vec![id, copied, external];
    expected.sort();
    assert_eq!(result.removed, expected);
    assert!(folders.list().is_empty());
    let notes = Notes::load(base.join("notes")).unwrap();
    assert!(
        notes.note(id).is_none() && notes.note(copied).is_none() && notes.note(external).is_none()
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn file_selection_validates_all_destinations_and_preserves_document_ids() {
    let base = temporary("selection");
    let data = base.join("data");
    fs::create_dir(&data).unwrap();
    let root = base.join("work");
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let config = data.join("workspace.json");
    write_json(&config, &[&root]).unwrap();
    let mut workspace = Workspace::load(config).unwrap();
    let mut folders = Folders::load_fixture(data.join("folders.json")).unwrap();
    let notes = RwLock::new(Notes::load(data.join("notes")).unwrap());
    let a = root.join("A.md");
    let b = root.join("B.md");
    fs::write(&a, "甲").unwrap();
    fs::write(&b, "乙").unwrap();
    let id = notes.write().unwrap().open_file(&a).unwrap();
    let external = target.join("B.md");
    fs::write(&external, "碰撞").unwrap();
    let move_files = |ws: &mut Workspace, fs: &mut Folders, ns: &RwLock<Notes>| {
        change_selection(
            ws,
            fs,
            ns,
            vec![a.clone(), b.clone()],
            Some(target.clone()),
            &mut FileResult::default(),
        )
    };
    assert!(move_files(&mut workspace, &mut folders, &notes).is_err());
    assert!(a.exists() && b.exists());
    assert_eq!(fs::read_to_string(&external).unwrap(), "碰撞");
    fs::remove_file(external).unwrap();
    move_files(&mut workspace, &mut folders, &notes).unwrap();
    assert!(!a.exists() && !b.exists());
    assert_eq!(
        notes.read().unwrap().note(id).unwrap().file,
        Some(target.join("A.md"))
    );
    assert_eq!(fs::read_to_string(target.join("B.md")).unwrap(), "乙");
    fs::remove_dir_all(base).unwrap();
}
