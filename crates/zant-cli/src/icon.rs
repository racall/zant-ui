//! The icon tool that turns `assets/icons/app-windows.png` into the Windows
//! app `.ico`.
//!
//!   zant-icon ico <app-windows.png> <out.ico>
//!
//! The app build script (`zant-build`) builds the executable's `.ico` while
//! compiling; this module has the same read / validate / write logic with the
//! command-line error messages, and is only exercised by its tests.

pub const USAGE: &str = "用法: zant-icon ico <in.png> <out.ico>";

/// Failure of `zant-icon`, with the message it prints.
#[derive(Debug)]
pub enum IconError {
    /// Nothing usable on the command line; exit code 2.
    Usage,
    Read {
        path: String,
        err: String,
    },
    Size {
        path: String,
        width: u32,
        height: u32,
    },
    Write(std::io::Error),
}

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconError::Usage => f.write_str(USAGE),
            IconError::Read { path, err } => write!(
                f,
                "zant-icon: {path}: 不能读取（{err}）；需要 8 位、非隔行扫描的 PNG"
            ),
            IconError::Size {
                path,
                width,
                height,
            } => write!(
                f,
                "zant-icon: {path} 是 {width}×{height}；Windows 应用图标需要正方形、至少 256×256"
            ),
            IconError::Write(err) => write!(f, "zant-icon: {err}"),
        }
    }
}

impl std::error::Error for IconError {}

/// Decodes `app-windows.png` and writes the `.ico` (16–256, one PNG per size).
pub fn ico(input: &str, output: &str) -> Result<(), IconError> {
    let data = std::fs::read(input).map_err(|err| IconError::Read {
        path: input.to_string(),
        err: err.to_string(),
    })?;
    let img = crate::png::decode(&data).map_err(|err| IconError::Read {
        path: input.to_string(),
        err: err.to_string(),
    })?;
    if img.width != img.height || img.width < 256 {
        return Err(IconError::Size {
            path: input.to_string(),
            width: img.width,
            height: img.height,
        });
    }
    let bytes = crate::png::build_ico(&img).map_err(|err| IconError::Read {
        path: input.to_string(),
        err: err.to_string(),
    })?;
    if let Some(parent) = std::path::Path::new(output).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(IconError::Write)?;
    }
    std::fs::write(output, bytes).map_err(IconError::Write)
}

/// The `zant-icon` entry point; returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    if args.len() >= 4 && args[1] == "ico" {
        match ico(&args[2], &args[3]) {
            Ok(()) => return 0,
            Err(err) => {
                eprintln!("{err}");
                return 1;
            }
        }
    }
    eprintln!("{USAGE}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/zant-cli-test")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn builds_ico_from_a_256_png() {
        let dir = temp_dir("icon-ok");
        let mut rgba = Vec::with_capacity(256 * 256 * 4);
        for i in 0..256 * 256 {
            rgba.extend_from_slice(&[(i % 255) as u8, 64, 200, 255]);
        }
        let img = crate::png::Image {
            width: 256,
            height: 256,
            rgba,
        };
        let png = crate::png::encode(&img).expect("encode");
        let input = dir.join("app-windows.png");
        let output = dir.join("app.ico");
        std::fs::write(&input, png).expect("write png");
        ico(input.to_str().unwrap(), output.to_str().unwrap()).expect("ico");
        let bytes = std::fs::read(&output).expect("read ico");
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 8);
    }

    #[test]
    fn rejects_small_and_broken_icons() {
        let dir = temp_dir("icon-bad");
        let input = dir.join("small.png");
        let img = crate::png::Image {
            width: 32,
            height: 32,
            rgba: vec![0; 32 * 32 * 4],
        };
        std::fs::write(&input, crate::png::encode(&img).expect("encode")).expect("write");
        let err = ico(input.to_str().unwrap(), dir.join("x.ico").to_str().unwrap())
            .expect_err("small icon");
        assert!(err.to_string().contains("至少 256×256"));
        let broken = dir.join("broken.png");
        std::fs::write(&broken, b"nope").expect("write");
        let err = ico(
            broken.to_str().unwrap(),
            dir.join("y.ico").to_str().unwrap(),
        )
        .expect_err("broken");
        assert!(err.to_string().contains("不能读取"));
        assert_eq!(run(&["zant-icon".into()]), 2);
        assert_eq!(
            run(&["zant-icon".into(), "nope".into(), "a".into(), "b".into()]),
            2
        );
    }
}
