//! App-store sources and install requests.
//!
//! A source is either a website index or a GitHub repository. Both deliver a
//! versioned index whose entries point at signed rumahl packages. The platform
//! keeps network access server-side so the shell never talks to third parties.
use crate::ShellIdentity;

/// One configured app source, as exposed to the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSource {
    pub id: String,
    /// `website` or `github`.
    pub kind: String,
    pub location: String,
    pub title: String,
    /// `ready` or `unavailable`.
    pub status: String,
    pub error: Option<String>,
}

/// One app advertised by a source. Metadata is untrusted until install verifies
/// the signed package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreApp {
    pub source_id: String,
    pub id: String,
    pub title: String,
    pub version: String,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub icon: Option<String>,
}

/// The merged live view of every configured source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreListing {
    pub sources: Vec<StoreSource>,
    pub apps: Vec<StoreApp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The request is malformed or the source index is not usable.
    InvalidRequest,
    /// The source could not be reached or the platform is unavailable.
    Unavailable,
}

/// Downloads and installs packages from websites and GitHub repositories.
pub trait StoreProvider: Send + Sync + 'static {
    fn list(&self, identity: ShellIdentity) -> Result<StoreListing, StoreError>;
    fn add_source(
        &self,
        identity: ShellIdentity,
        kind: &str,
        location: &str,
    ) -> Result<StoreSource, StoreError>;
    fn remove_source(&self, identity: ShellIdentity, id: &str) -> Result<(), StoreError>;
    /// Fetches, verifies and installs the selected app package.
    fn install(
        &self,
        identity: ShellIdentity,
        source_id: &str,
        app_id: &str,
    ) -> Result<crate::ImportedApp, StoreError>;
}
