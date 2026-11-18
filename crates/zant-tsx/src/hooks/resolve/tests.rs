//! Unit tests.

use super::*;

#[test]
fn relative_paths_between_directories() {
    assert_eq!("../assets/x.png", text(&relative(b"src", b"assets/x.png")));
    assert_eq!("./AppStore", text(&relative(b"src", b"src/AppStore")));
    assert_eq!("./hooks/useX", text(&relative(b"src", b"src/hooks/useX")));
    assert_eq!(
        "../AppStore",
        text(&relative(b"src/hooks", b"src/AppStore"))
    );
    assert_eq!(
        "../../assets/x.png",
        text(&source_for(
            b"src/hooks/useTray.tsx",
            b"src/a/App.tsx",
            b"../../assets/x.png"
        ))
    );
    assert_eq!(
        "../assets/x.png",
        text(&source_for(
            b"src/hooks/useTray.tsx",
            b"src/App.tsx",
            b"../../assets/x.png"
        ))
    );
}

// `basename` / `dirname` follow POSIX semantics (trailing slashes, root).
#[test]
fn path_helpers_follow_posix_semantics() {
    assert_eq!(b"basename.ext", basename(b"/dir/basename.ext"));
    assert_eq!(b"aaa", basename(b"/aaa/"));
    assert_eq!(b"", basename(b"/"));
    assert_eq!(None, dirname(b"a"));
    assert_eq!(b"/", dirname(b"/a").unwrap());
    assert_eq!(b"src/hooks", dirname(b"src/hooks/useX.tsx").unwrap());
    assert_eq!(None, dirname(b"/"));
    assert_eq!(b".png", extension(b".image.png"));
    assert_eq!(b"", extension(b".gitignore"));
    assert!(is_asset(b"../assets/tray.png"));
    assert!(!is_asset(b"./Row.tsx"));
    assert!(!is_asset(b"zant"));
    assert!(!is_asset(b"./.hidden"));
    assert_eq!(
        b"assets/x.png".as_slice(),
        normalized(b"src/hooks/../../assets/x.png").as_slice()
    );
    assert_eq!(b".".as_slice(), normalized(b"src/..").as_slice());
    assert_eq!(b"..".as_slice(), normalized(b"..").as_slice());
    assert_eq!(b"../a".as_slice(), normalized(b"../a").as_slice());
}
