use std::{fs, io, path::Path};

/// Writes to a sibling first, so a failed export never leaves a partial
/// destination. `rename` replaces an existing destination atomically on macOS.
pub fn write_atomically(destination: &Path, content: &str) -> io::Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "export destination has no parent folder",
        )
    })?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "export destination has no filename",
            )
        })?;
    let temporary = parent.join(format!(
        ".{file_name}.{}.nodepad-export-tmp",
        uuid::Uuid::new_v4()
    ));

    let result = (|| {
        fs::write(&temporary, content)?;
        fs::rename(&temporary, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn default_filename(workspace_name: &str) -> String {
    let stem: String = workspace_name
        .chars()
        .map(|character| match character {
            '/' | ':' | '\\' | '\0' => '-',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{}.md",
        if stem.is_empty() {
            "Thinking Workspace"
        } else {
            &stem
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nodepad-export-{name}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn sanitizes_a_unicode_workspace_name_without_losing_safe_characters() {
        assert_eq!(
            default_filename("Café / notes: 日本語"),
            "Café - notes- 日本語.md"
        );
    }

    #[test]
    fn atomically_replaces_an_existing_destination() {
        let destination = temporary_path("replace");
        fs::write(&destination, "old").unwrap();
        write_atomically(&destination, "new").unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
        assert!(!destination
            .with_file_name(format!(
                ".{}.nodepad-export-tmp",
                destination.file_name().unwrap().to_string_lossy()
            ))
            .exists());
        fs::remove_file(destination).unwrap();
    }

    #[test]
    fn write_failure_leaves_no_partial_destination() {
        let destination = temporary_path("missing-parent").join("workspace.md");
        assert!(write_atomically(&destination, "content").is_err());
        assert!(!destination.exists());
    }
}
