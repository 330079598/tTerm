use super::*;

#[gpui_kit::test]
fn local_shell_menu_cancels_then_launches_the_saved_program(cx: &mut TestAppContext) {
    #[cfg(unix)]
    let (path, args) = ("/bin/sh", "-c \"echo PICKED_PROGRAM; read answer\"");
    #[cfg(windows)]
    let (path, args) = ("cmd.exe", "/d /q /k echo PICKED_PROGRAM");
    let harness = harness(
        cx,
        AppConfig {
            local_shells: vec![LocalShell {
                id: "picked".into(),
                name: "Picked".into(),
                path: path.into(),
                args: args.into(),
            }],
            ..Default::default()
        },
    );
    let root = harness.root.clone();
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "escape");
    harness.visual.update(|window, cx| {
        assert_eq!(root.read(cx).tabs.len(), 2);
        assert_eq!(root.read(cx).tabs.active_id(), 1);
        assert!(
            root.read(cx)
                .tabs
                .active()
                .view
                .read(cx)
                .focus_handle()
                .is_focused(window)
        );
    });
    click(harness.visual, "new-shell-menu");
    press(harness.visual, "down down enter");
    let model = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert!(Rc::ptr_eq(&harness.first, &tab_binding(workspace, 1)));
        let binding = tab_binding(workspace, workspace.tabs.active_id());
        binding.session().expect("selected local PTY").model()
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !output(&model).contains("PICKED_PROGRAM") {
        assert!(
            Instant::now() < deadline,
            "selected program output: {}",
            output(&model)
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    harness.visual.update(|window, _| window.remove_window());
}
