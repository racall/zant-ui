//! Download file extensions for code fences.

const EXTENSION_MAP: &[(&str, &str)] = &[
    ("bash", "sh"),
    ("sh", "sh"),
    ("shell", "sh"),
    ("zsh", "zsh"),
    ("console", "sh"),
    ("c", "c"),
    ("c++", "cpp"),
    ("cpp", "cpp"),
    ("c#", "cs"),
    ("cs", "cs"),
    ("csharp", "cs"),
    ("css", "css"),
    ("scss", "scss"),
    ("csv", "csv"),
    ("dart", "dart"),
    ("diff", "diff"),
    ("docker", "dockerfile"),
    ("dockerfile", "dockerfile"),
    ("go", "go"),
    ("golang", "go"),
    ("html", "html"),
    ("java", "java"),
    ("javascript", "js"),
    ("js", "js"),
    ("jsx", "jsx"),
    ("json", "json"),
    ("kotlin", "kt"),
    ("kt", "kt"),
    ("lua", "lua"),
    ("markdown", "md"),
    ("md", "md"),
    ("mermaid", "mmd"),
    ("objc", "m"),
    ("php", "php"),
    ("python", "py"),
    ("py", "py"),
    ("ruby", "rb"),
    ("rb", "rb"),
    ("rust", "rs"),
    ("rs", "rs"),
    ("sql", "sql"),
    ("swift", "swift"),
    ("toml", "toml"),
    ("ts", "ts"),
    ("typescript", "ts"),
    ("tsx", "tsx"),
    ("xml", "xml"),
    ("yaml", "yaml"),
    ("yml", "yml"),
    ("zig", "zig"),
    ("latex", "tex"),
    ("tex", "tex"),
    ("makefile", "mk"),
    ("vue", "vue"),
    ("svelte", "svelte"),
];

/// Download file extension for a fence language (Streamdown
/// `languageExtensionMap`, common subset). Unknown → "txt".
pub fn extension(lang: &str) -> &'static str {
    for (name, ext) in EXTENSION_MAP {
        if lang.eq_ignore_ascii_case(name) {
            return ext;
        }
    }
    "txt"
}
