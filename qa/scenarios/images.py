"""Exercise the editor's picker, compression and failure path against the desktop IPC contract."""
from io import BytesIO

from PIL import Image
from playwright.sync_api import expect


def check_image_settings_and_picker(browser, url, ipc):
    context = browser.new_context()
    context.add_init_script(ipc + """
      const native = window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.convertFileSrc = () => '/fixture.png';
      window.__TAURI_INTERNALS__.invoke = async (cmd, args, options) => {
        if (cmd === 'save_image_settings') {
          const saved = {config:args.config, revision:args.revision+1, folder:null};
          localStorage.setItem('imageSettings', JSON.stringify(saved));
          return saved;
        }
        if (cmd === 'read_image_settings' && localStorage.imageSettings) return JSON.parse(localStorage.imageSettings);
        if (cmd === 'image_key') return args.address === 'https://upload.example.test/images';
        if (cmd === 'choose_folder') return '/photos';
        if (cmd === 'workspace_roots') return ['/work', '/work/' + 'LongPath'.repeat(35)];
        if (cmd === 'list_folders') return [{id:10,name:'Meetings',path:'/archive/meetings',description:'',classify:false}];
        if (cmd === 'image_options') return {revision:3, mode:'relative', compress:JSON.parse(localStorage.imageSettings).config.compress};
        if (cmd === 'store_note_image') {
          if (window.failImage) throw new Error('fixture upload failed');
          const bytes = new Uint8Array(args);
          window.upload = {size: bytes.length, headers: options.headers,
            width: (await createImageBitmap(new Blob([bytes]))).width};
          return '_assets/fixture.png';
        }
        if (cmd === 'save_note') { window.savedImageNote = note.markdown = args.markdown; return ++note.version; }
        return native(cmd, args, options);
      };
    """)
    settings = context.new_page()
    settings.goto(url + "?view=settings")
    def open_images():
        settings.get_by_role("button", name="图片管理", exact=True).click()

    open_images()
    form = settings.locator(".image-settings-form")
    save = form.get_by_role("button", name="保存设置")
    status = form.locator(".image-save-bar [role=status]")
    preview = form.locator(".image-settings-defaults > .image-preview")
    expect(form.locator("#image-subfolder")).to_have_value("_assets")
    expect(form.locator("#image-directory")).to_be_hidden()
    expect(form.locator("#image-endpoint")).to_be_hidden()
    form.locator("#image-group").check()
    expect(preview).to_contain_text("notes/_assets/周报/image.png")
    expect(status).to_have_text("有未保存的更改")
    form.get_by_role("button", name="放弃更改").click()
    expect(form.locator("#image-group")).not_to_be_checked()
    expect(preview).to_contain_text("notes/_assets/image.png")
    expect(save).to_be_disabled()

    form.locator("#image-mode-fixed").check()
    form.get_by_role("button", name="选择…", exact=True).click()
    expect(form.locator("#image-directory")).to_have_value("/photos")
    expect(preview).to_contain_text("图片目录/image.png")
    expect(preview).not_to_contain_text("/photos")
    form.locator("#image-mode-http").check()
    form.locator("#image-bearer").check()
    expect(form.get_by_role("button", name="配置密钥…")).to_be_disabled()
    form.locator("#image-endpoint").fill("https://upload.example.test/images")
    expect(form.get_by_text("此服务已配置密钥。")).to_be_visible()
    expect(preview).not_to_contain_text("https://upload.example.test/images")
    form.locator("#image-pointer").fill("/data/url")
    form.locator("#image-compress").check()

    form.get_by_text("配置其他保存方式", exact=True).click()
    other = form.locator(".image-other-methods")
    other.get_by_role("button", name="固定目录", exact=True).click()
    expect(form.locator("#image-directory")).to_be_visible()
    expect(form.locator("#image-mode-http")).to_be_checked()
    other.get_by_text("配置其他保存方式", exact=True).click()

    dialog = settings.get_by_role("dialog", name="此文件夹的图片保存方式")
    apply = dialog.get_by_role("button", name="应用", exact=True)
    add = form.get_by_text("添加目录规则", exact=True)

    def open_folder(name):
        add.click()
        form.locator(".image-folder-choice").get_by_text(name, exact=True).click()
        expect(dialog.locator(".image-folder-label strong").first).to_have_text({"Meetings": "meetings"}.get(name, name))

    open_folder("work")
    expect(form.locator(".image-rule")).to_have_count(0)
    dialog.get_by_role("button", name="取消", exact=True).click()
    expect(form.locator(".image-rule")).to_have_count(0)
    form.locator(".image-folder-choice").get_by_text("work", exact=True).click()
    dialog.get_by_label("保存方式", exact=True).select_option("fixed")
    dialog.get_by_label("图片目录来源").select_option("relative")
    dialog.get_by_label("相对位置", exact=True).fill("../access")
    expect(dialog.locator(".image-preview")).to_contain_text("assets/image.png")
    expect(dialog.locator(".image-preview")).not_to_contain_text("access")
    expect(dialog).not_to_contain_text("/work")
    apply.click()
    expect(dialog).to_have_count(0)
    first = form.get_by_role("button", name="编辑 work 的图片规则", exact=True)
    expect(first).to_be_focused()
    assert settings.evaluate("localStorage.getItem('imageSettings')") is None

    open_folder("Meetings")
    dialog.get_by_label("保存方式", exact=True).select_option("http")
    apply.click()
    open_folder("其他")
    dialog.get_by_label("保存方式", exact=True).select_option("relative")
    dialog.get_by_label("图片子文件夹名称").fill("media/notes")
    apply.click()
    inbox = form.get_by_role("button", name="编辑 其他 的图片规则", exact=True)
    add.click()
    for preset in ("default", "ink", "material", "blueprint"):
        settings.evaluate("""async themePreset => testEvent('desktop:appearance-changed',
          {...await __TAURI_INTERNALS__.invoke('read_appearance'), themePreset, mode:'dark', baseSize:18})""", preset)
        for width in (475, 680):
            settings.set_viewport_size({"width": width, "height": 510})
            bounds = form.evaluate("""form => {
              const panel = form.closest('.settings-content');
              panel.scrollTop = 0;
              const edge = panel.getBoundingClientRect().left + panel.clientWidth;
              const outside = [...form.querySelectorAll('*')].filter(node =>
                node.getBoundingClientRect().width && node.getBoundingClientRect().right > edge + 1);
              const footer = form.querySelector('.image-save-bar').getBoundingClientRect();
              return [panel.scrollWidth - panel.clientWidth,
                outside.map(node => [node.id || node.className || node.tagName, node.clientWidth]),
                footer.bottom <= panel.getBoundingClientRect().bottom + 1];
            }""")
            assert bounds[0] <= 1 and not bounds[1] and bounds[2], (preset, width, bounds)
    save.click()
    expect(status).to_have_text("已保存")
    settings.reload()
    open_images()
    expect(form.locator("#image-mode-http")).to_be_checked()
    expect(form.locator("#image-pointer")).to_have_value("/data/url")
    expect(form.locator("#image-compress")).to_be_checked()
    first.click()
    expect(dialog.get_by_label("保存方式", exact=True)).to_have_value("fixed")
    expect(dialog.get_by_label("相对位置", exact=True)).to_have_value("../access")
    dialog.get_by_role("button", name="取消", exact=True).click()
    inbox.click()
    expect(dialog.get_by_label("图片子文件夹名称")).to_have_value("media/notes")
    dialog.get_by_role("button", name="取消", exact=True).click()
    first.click()
    dialog.get_by_role("button", name="规则选项", exact=True).click()
    dialog.get_by_role("button", name="恢复继承", exact=True).click()
    apply.click()
    save.click()
    expect(status).to_have_text("已保存")
    rules = settings.evaluate("JSON.parse(localStorage.imageSettings).config.rules")
    assert [r["path"] for r in rules] == ["/archive/meetings", "/tmp/Jotiva/其他"]
    page = context.new_page()
    page.goto(url)
    pixels = BytesIO()
    Image.new("RGB", (241, 128), "teal").save(pixels, format="PNG", compress_level=0)
    data = pixels.getvalue()
    page.route("**/fixture.png", lambda r: r.fulfill(body=data, content_type="image/png"))

    def select_image():
        page.locator('[data-jot-toolbar-command="toolbar.open-image"]').click()
        with page.expect_file_chooser() as chooser:
            # WebKit 0.4.0 closes this upstream popover on mouse focusout before click.
            page.get_by_role("button", name="选择本地图片…").press("Enter")
        chooser.value.set_files({"name": "test.png", "mimeType": "image/png", "buffer": data})

    content = page.locator(".cm-content")
    content.focus()
    select_image()
    page.wait_for_function("window.savedImageNote?.includes('_assets/fixture.png')")
    upload = page.evaluate("window.upload")
    assert upload["width"] == 241 and 0 < upload["size"] < len(data)
    assert upload["headers"]["x-jot-revision"] == "3" and upload["headers"]["x-jot-note"] == "1"
    page.evaluate("window.priorVersion = window.savedImageNote; window.failImage = true")
    select_image()
    expect(page.get_by_text("fixture upload failed", exact=True)).to_be_visible()
    assert page.evaluate("window.priorVersion === window.savedImageNote")
    context.close()
    print("Image preferences persisted. File picker compressed without resizing; a failed save left the Markdown intact.")
