//! Composes a user-scoped shell snapshot from replaceable read sources.
//!
//! Every source must participate in the same revision domain. The revision
//! source must change whenever any projected value changes; reading it twice
//! detects an update while the snapshot is being assembled. An adapter that
//! cannot guarantee that invariant should instead expose one transactional
//! read model behind `ShellSnapshotProvider`.

use std::error::Error;
use std::fmt;

use rumahl_ui_contracts::{
    ExtensionContribution, ShellSnapshot, ShellSnapshotError, ShellSystemStatus, ShellTheme,
    ShellUser,
};

use crate::{ShellSnapshotProvider, ShellSnapshotSubject};

pub type BoxedShellSourceError = Box<dyn Error + Send + Sync>;

pub trait ShellUserSource {
    fn load_user(&self, subject: &ShellSnapshotSubject)
    -> Result<ShellUser, BoxedShellSourceError>;
}

pub trait ShellThemeSource {
    fn load_theme(
        &self,
        subject: &ShellSnapshotSubject,
    ) -> Result<ShellTheme, BoxedShellSourceError>;
}

pub trait ShellStatusSource {
    fn load_status(
        &self,
        subject: &ShellSnapshotSubject,
    ) -> Result<ShellSystemStatus, BoxedShellSourceError>;
}

pub trait ShellContributionSource {
    fn load_contributions(
        &self,
        subject: &ShellSnapshotSubject,
    ) -> Result<Vec<ExtensionContribution>, BoxedShellSourceError>;
}

pub trait ShellRevisionSource {
    fn load_revision(
        &self,
        subject: &ShellSnapshotSubject,
    ) -> Result<String, BoxedShellSourceError>;
}

pub struct ShellSnapshotComposer<U, T, S, C, R> {
    shell_build_id: String,
    users: U,
    themes: T,
    statuses: S,
    contributions: C,
    revisions: R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellSnapshotSourceKind {
    User,
    Theme,
    Status,
    Contribution,
    Revision,
}

pub enum ShellSnapshotCompositionError {
    Source {
        kind: ShellSnapshotSourceKind,
        source: BoxedShellSourceError,
    },
    ChangedDuringRead,
    InvalidSnapshot(ShellSnapshotError),
}

impl<U, T, S, C, R> ShellSnapshotComposer<U, T, S, C, R> {
    pub fn new(
        shell_build_id: impl Into<String>,
        users: U,
        themes: T,
        statuses: S,
        contributions: C,
        revisions: R,
    ) -> Self {
        Self {
            shell_build_id: shell_build_id.into(),
            users,
            themes,
            statuses,
            contributions,
            revisions,
        }
    }
}

impl<U, T, S, C, R> ShellSnapshotProvider for ShellSnapshotComposer<U, T, S, C, R>
where
    U: ShellUserSource,
    T: ShellThemeSource,
    S: ShellStatusSource,
    C: ShellContributionSource,
    R: ShellRevisionSource,
{
    type Error = ShellSnapshotCompositionError;

    fn load_for_user(&self, subject: &ShellSnapshotSubject) -> Result<ShellSnapshot, Self::Error> {
        let revision = self
            .revisions
            .load_revision(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::Revision, source))?;
        let user = self
            .users
            .load_user(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::User, source))?;
        let theme = self
            .themes
            .load_theme(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::Theme, source))?;
        let status = self
            .statuses
            .load_status(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::Status, source))?;
        let contributions = self
            .contributions
            .load_contributions(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::Contribution, source))?;
        let current_revision = self
            .revisions
            .load_revision(subject)
            .map_err(|source| source_error(ShellSnapshotSourceKind::Revision, source))?;
        if revision != current_revision {
            return Err(ShellSnapshotCompositionError::ChangedDuringRead);
        }

        ShellSnapshot::new(
            self.shell_build_id.clone(),
            revision,
            user,
            theme,
            status,
            contributions,
        )
        .map_err(ShellSnapshotCompositionError::InvalidSnapshot)
    }
}

fn source_error(
    kind: ShellSnapshotSourceKind,
    source: BoxedShellSourceError,
) -> ShellSnapshotCompositionError {
    ShellSnapshotCompositionError::Source { kind, source }
}

impl fmt::Debug for ShellSnapshotCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { kind, .. } => formatter
                .debug_struct("Source")
                .field("kind", kind)
                .finish(),
            Self::ChangedDuringRead => formatter.write_str("ChangedDuringRead"),
            Self::InvalidSnapshot(error) => formatter
                .debug_tuple("InvalidSnapshot")
                .field(error)
                .finish(),
        }
    }
}

impl fmt::Display for ShellSnapshotCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { .. } => write!(formatter, "shell snapshot source is unavailable"),
            Self::ChangedDuringRead => write!(formatter, "shell snapshot changed during reading"),
            Self::InvalidSnapshot(_) => write!(formatter, "shell snapshot data is invalid"),
        }
    }
}

impl Error for ShellSnapshotCompositionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source { source, .. } => Some(source.as_ref()),
            Self::ChangedDuringRead => None,
            Self::InvalidSnapshot(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io;

    use rumahl_core::{CorrelationId, UserId};
    use rumahl_ui_contracts::{ShellSystemStatus, SystemProtectionStatus, WindowChromeVariant};

    use super::*;

    struct Sources {
        expected_user: UserId,
        revision_reads: Cell<u8>,
        change_revision: bool,
        fail_theme: bool,
    }

    impl ShellUserSource for &Sources {
        fn load_user(
            &self,
            subject: &ShellSnapshotSubject,
        ) -> Result<ShellUser, BoxedShellSourceError> {
            if subject.user_id() != &self.expected_user {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "wrong user").into());
            }
            Ok(ShellUser::new("Ada", "de-DE")?)
        }
    }

    impl ShellThemeSource for &Sources {
        fn load_theme(
            &self,
            _: &ShellSnapshotSubject,
        ) -> Result<ShellTheme, BoxedShellSourceError> {
            if self.fail_theme {
                return Err(io::Error::other("internal theme failure").into());
            }
            Ok(ShellTheme::new(
                "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
                WindowChromeVariant::Standard,
            )?)
        }
    }

    impl ShellStatusSource for &Sources {
        fn load_status(
            &self,
            _: &ShellSnapshotSubject,
        ) -> Result<ShellSystemStatus, BoxedShellSourceError> {
            Ok(ShellSystemStatus::new(
                SystemProtectionStatus::Active,
                2,
                1_790_106_120_000,
                None,
            )?)
        }
    }

    impl ShellContributionSource for &Sources {
        fn load_contributions(
            &self,
            _: &ShellSnapshotSubject,
        ) -> Result<Vec<ExtensionContribution>, BoxedShellSourceError> {
            Ok(vec![ExtensionContribution::command(
                "com.example.notes.open",
                "Open notes",
                "com.example.notes.open",
            )?])
        }
    }

    impl ShellRevisionSource for &Sources {
        fn load_revision(&self, _: &ShellSnapshotSubject) -> Result<String, BoxedShellSourceError> {
            let reads = self.revision_reads.get() + 1;
            self.revision_reads.set(reads);
            Ok(if self.change_revision && reads > 1 {
                "revision-2"
            } else {
                "revision-1"
            }
            .to_owned())
        }
    }

    fn sources(expected_user: UserId) -> Sources {
        Sources {
            expected_user,
            revision_reads: Cell::new(0),
            change_revision: false,
            fail_theme: false,
        }
    }

    fn compose(
        sources: &Sources,
    ) -> ShellSnapshotComposer<&Sources, &Sources, &Sources, &Sources, &Sources> {
        ShellSnapshotComposer::new(
            "shell-build-001",
            sources,
            sources,
            sources,
            sources,
            sources,
        )
    }

    #[test]
    fn composes_all_sources_for_the_authenticated_user() {
        let user = UserId::new();
        let sources = sources(user);
        let snapshot = compose(&sources)
            .load_for_user(&ShellSnapshotSubject::new(user, CorrelationId::new()))
            .unwrap();

        assert_eq!(snapshot.user().display_name(), "Ada");
        assert_eq!(snapshot.user().locale(), "de-DE");
        assert_eq!(snapshot.system_status().installed_app_count(), 2);
        assert_eq!(snapshot.contributions().len(), 1);
        assert_eq!(snapshot.revision(), "revision-1");
        assert_eq!(sources.revision_reads.get(), 2);
    }

    #[test]
    fn rejects_an_update_during_assembly() {
        let user = UserId::new();
        let mut sources = sources(user);
        sources.change_revision = true;

        let result =
            compose(&sources).load_for_user(&ShellSnapshotSubject::new(user, CorrelationId::new()));

        assert!(matches!(
            result,
            Err(ShellSnapshotCompositionError::ChangedDuringRead)
        ));
    }

    #[test]
    fn identifies_failing_source_without_exposing_its_details() {
        let user = UserId::new();
        let mut sources = sources(user);
        sources.fail_theme = true;

        let error = compose(&sources)
            .load_for_user(&ShellSnapshotSubject::new(user, CorrelationId::new()))
            .unwrap_err();

        assert!(matches!(
            error,
            ShellSnapshotCompositionError::Source {
                kind: ShellSnapshotSourceKind::Theme,
                ..
            }
        ));
        assert_eq!(error.to_string(), "shell snapshot source is unavailable");
        assert!(!format!("{error:?}").contains("internal theme failure"));
        assert_eq!(sources.revision_reads.get(), 1);
    }
}
