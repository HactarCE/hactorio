//! Locating the game and rebuilding the registry from its data stage.

use std::path::PathBuf;
use std::process::Command;

use crate::icons::IconSource;
use crate::{Error, Locale, Registry, postprocess};

/// An installed Factorio, with the directories an import needs.
#[derive(Debug, Clone)]
pub struct GameInstall {
    pub binary: PathBuf,
    pub data_dir: PathBuf,
    pub mod_dir: PathBuf,
    /// Where the game writes `--dump-data` output.
    pub script_output: PathBuf,
}

impl GameInstall {
    pub fn icon_source(&self) -> IconSource {
        IconSource::new(self.data_dir.clone(), self.mod_dir.clone())
    }

    /// Runs the data-stage dumps and post-processes the result.
    pub fn dump(&self) -> Result<Registry, Error> {
        let status = Command::new(&self.binary)
            .arg("--dump-data")
            .arg("--dump-prototype-locale")
            .status()?;
        if !status.success() {
            return Err(Error::GameData(format!(
                "`{}` exited with {status}",
                self.binary.display()
            )));
        }
        let raw = std::fs::read_to_string(self.script_output.join("data-raw-dump.json"))?;
        let locale = Locale::load(&self.script_output);
        postprocess(&raw, &locale)
    }
}

/// Finds Factorio in the usual Steam locations for this platform.
pub fn locate() -> Option<GameInstall> {
    #[cfg(target_os = "macos")]
    {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        let contents = home
            .join("Library/Application Support/Steam/steamapps/common/Factorio/factorio.app/Contents");
        let user_data = home.join("Library/Application Support/factorio");
        install(contents.join("MacOS/factorio"), contents.join("data"), user_data)
    }
    #[cfg(target_os = "linux")]
    {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        let candidates = [
            home.join(".steam/steam/steamapps/common/Factorio"),
            home.join(".local/share/Steam/steamapps/common/Factorio"),
        ];
        candidates.into_iter().find_map(|root| {
            install(
                root.join("bin/x64/factorio"),
                root.join("data"),
                home.join(".factorio"),
            )
        })
    }
    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)?;
        let user_data = PathBuf::from(std::env::var_os("APPDATA")?).join("Factorio");
        let root = program_files.join("Steam/steamapps/common/Factorio");
        install(
            root.join("bin/x64/factorio.exe"),
            root.join("data"),
            user_data,
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

fn install(binary: PathBuf, data_dir: PathBuf, user_data: PathBuf) -> Option<GameInstall> {
    if !binary.is_file() || !data_dir.is_dir() {
        return None;
    }
    let mod_dir = user_data.join("mods");
    Some(GameInstall {
        script_output: user_data.join("script-output"),
        binary,
        data_dir,
        mod_dir,
    })
}
