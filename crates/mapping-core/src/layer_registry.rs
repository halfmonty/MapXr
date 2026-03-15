use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::ProfileError;
use crate::types::Profile;

/// A unique layer identifier string (matches `Profile::layer_id`).
pub type LayerId = String;

/// Loads and caches all valid profile `.json` files from a directory.
///
/// Profiles that fail to load are silently skipped — only successfully
/// validated profiles are inserted into the map. Call [`LayerRegistry::load_errors`]
/// to inspect which files failed and why.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use mapping_core::LayerRegistry;
///
/// let mut registry = LayerRegistry::new(Path::new("/path/to/profiles"));
/// registry.reload().unwrap();
/// if let Some(profile) = registry.get("base") {
///     println!("loaded: {}", profile.name);
/// }
/// ```
pub struct LayerRegistry {
    /// Directory that contains the `.json` profile files.
    dir: PathBuf,
    /// Successfully loaded profiles, keyed by `layer_id`.
    profiles: HashMap<LayerId, Profile>,
    /// Files that failed to load, with their errors.
    errors: Vec<(PathBuf, ProfileError)>,
}

impl LayerRegistry {
    /// Create a new registry pointing at `dir`. Does not scan the directory
    /// until [`reload`](LayerRegistry::reload) is called.
    pub fn new(dir: impl Into<PathBuf> + AsRef<std::path::Path>) -> Self {
        Self {
            dir: dir.into(),
            profiles: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Scan the directory, load every `.json` file, and rebuild the registry.
    ///
    /// Valid profiles replace any previously loaded profile with the same
    /// `layer_id`. Previously loaded profiles whose files have been removed
    /// are also dropped. Files that fail to load are recorded in the error
    /// list and do not affect already-loaded profiles from other files.
    ///
    /// Returns `Err` only if the directory itself cannot be read.
    pub fn reload(&mut self) -> Result<(), std::io::Error> {
        self.profiles.clear();
        self.errors.clear();

        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match Profile::load(&path) {
                Ok(profile) => {
                    self.profiles.insert(profile.layer_id.clone(), profile);
                }
                Err(err) => {
                    self.errors.push((path, err));
                }
            }
        }
        Ok(())
    }

    /// Return a reference to the profile with the given `layer_id`, if loaded.
    pub fn get(&self, layer_id: &str) -> Option<&Profile> {
        self.profiles.get(layer_id)
    }

    /// Return an iterator over all successfully loaded profiles.
    pub fn profiles(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.values()
    }

    /// Return the list of files that failed to load and their errors.
    pub fn load_errors(&self) -> &[(PathBuf, ProfileError)] {
        &self.errors
    }

    /// Return the number of successfully loaded profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Return `true` if no profiles are loaded.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}
