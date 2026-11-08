use super::transfer::{self, Folder, NAME};
use std::io::{Seek, SeekFrom, Write};

#[test]
fn delayed_large_transfer_preserves_conflicts_and_detects_corruption() {
    let folder = Folder::new().unwrap();
    let cleanup = folder.0.clone();
    assert!(
        !folder.0.join(NAME).exists(),
        "No download before the trigger"
    );
    let generated = transfer::generate(&folder).unwrap();
    let target = folder.0.join("destination");
    std::fs::create_dir(&target).unwrap();
    let target = transfer::validate_target(&target).unwrap();
    println!("{}", transfer::publish(&generated, &target).unwrap());
    assert!(transfer::publish(&generated, &target).is_err());
    transfer::verify(&target.join(NAME)).unwrap();
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .open(target.join(NAME))
        .unwrap();
    output.seek(SeekFrom::Start(199_999_999)).unwrap();
    output.write_all(&[255]).unwrap();
    drop(output);
    assert!(
        transfer::verify(&target.join(NAME))
            .unwrap_err()
            .contains("199999999")
    );
    drop(generated);
    drop(folder);
    assert!(
        !cleanup.exists(),
        "Per-drag staging and test target are cleaned up"
    );
}

#[test]
fn refuses_nonempty_or_outside_temp_target() {
    let folder = Folder::new().unwrap();
    assert!(transfer::validate_target(&std::env::temp_dir()).is_err());
    assert!(transfer::validate_target(&std::env::current_dir().unwrap()).is_err());
    std::fs::write(folder.0.join("unrelated"), b"preserve").unwrap();
    assert!(transfer::validate_target(&folder.0).is_err());
    assert_eq!(
        std::fs::read(folder.0.join("unrelated")).unwrap(),
        b"preserve"
    );
}

#[cfg(windows)]
#[test]
fn cancelled_native_staging_does_not_generate_or_delete_changed_copy() {
    let target = Folder::new().unwrap();
    let mut drag = super::native::Drag::prepare(&target.0).unwrap();
    assert!(!drag.folder.0.join(NAME).exists());
    assert!(!drag.finish().unwrap());
    let placeholder = std::fs::read_dir(&drag.folder.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    let copy = target.0.join(placeholder);
    std::fs::write(&copy, b"changed").unwrap();
    assert!(drag.finish().is_err());
    assert_eq!(std::fs::read(&copy).unwrap(), b"changed");
}

#[cfg(windows)]
#[test]
fn shell_data_object_offers_real_placeholder_as_hdrop() {
    use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
    use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

    // SAFETY: this test owns its COM apartment on the current test thread,
    // drops every COM reference before balancing initialization, and never starts a drag.
    unsafe {
        OleInitialize(None).unwrap();
        let folder = Folder::new().unwrap();
        let placeholder = folder.0.join("placeholder.bin");
        std::fs::write(&placeholder, []).unwrap();
        let data = super::native::data_object(&placeholder).unwrap();
        let format = FORMATETC {
            cfFormat: 15, // CF_HDROP is the Shell's existing-file format.
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
            ..Default::default()
        };
        assert!(data.QueryGetData(&format).is_ok());
        assert!(!folder.0.join(NAME).exists());
        drop(data);
        OleUninitialize();
    }
}
