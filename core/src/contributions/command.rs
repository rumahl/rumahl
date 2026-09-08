use std::error::Error;
use std::fmt;

use crate::Identity;

use super::{CommandAction, Contribution, ContributionError, ContributionId, ContributionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContribution {
    contribution: Contribution,
    title: String,
    action: CommandAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandContributionError {
    EmptyTitle,
    TitleTooLong,
    InvalidContribution(ContributionError),
}

impl CommandContribution {
    pub fn new(
        id: ContributionId,
        owner: Identity,
        title: impl Into<String>,
        action: CommandAction,
    ) -> Result<Self, CommandContributionError> {
        let title = title.into();
        let title = title.trim();

        if title.is_empty() {
            return Err(CommandContributionError::EmptyTitle);
        }

        if title.chars().count() > 120 {
            return Err(CommandContributionError::TitleTooLong);
        }

        let kind =
            ContributionKind::parse("command").expect("command is a valid contribution kind");

        let contribution = Contribution::new(id, owner, kind)
            .map_err(CommandContributionError::InvalidContribution)?;

        Ok(Self {
            contribution,
            title: title.to_owned(),
            action,
        })
    }

    pub fn contribution(&self) -> &Contribution {
        &self.contribution
    }

    pub fn id(&self) -> &ContributionId {
        self.contribution.id()
    }

    pub fn owner(&self) -> &Identity {
        self.contribution.owner()
    }

    pub fn kind(&self) -> &ContributionKind {
        self.contribution.kind()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn action(&self) -> &CommandAction {
        &self.action
    }
}

impl fmt::Display for CommandContributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => {
                write!(f, "command title cannot be empty")
            }

            Self::TitleTooLong => {
                write!(f, "command title cannot exceed 120 characters")
            }

            Self::InvalidContribution(error) => {
                write!(f, "invalid command contribution: {error}")
            }
        }
    }
}

impl Error for CommandContributionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidContribution(error) => Some(error),

            Self::EmptyTitle | Self::TitleTooLong => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        AppId, AppIdentity, CapabilityId, InstallationId, PublisherId, UserId, UserIdentity,
    };

    fn create_note_action() -> CommandAction {
        CommandAction::invoke_capability(
            CapabilityId::parse("com.rumahl.notes.create-note").unwrap(),
        )
    }

    fn notes_app() -> AppIdentity {
        AppIdentity::new(
            AppId::parse("com.rumahl.notes").unwrap(),
            InstallationId::new(),
            PublisherId::parse("com.rumahl").unwrap(),
        )
    }

    #[test]
    fn creates_command_contribution() {
        let command = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            notes_app().into(),
            "New note",
            create_note_action(),
        )
        .unwrap();

        assert_eq!(command.title(), "New note");

        assert_eq!(command.kind().as_str(), "command");

        assert_eq!(command.id().as_str(), "com.rumahl.notes.new-note");
    }

    #[test]
    fn trims_command_title() {
        let command = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            notes_app().into(),
            "  New note  ",
            create_note_action(),
        )
        .unwrap();

        assert_eq!(command.title(), "New note");
    }

    #[test]
    fn rejects_empty_title() {
        let result = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            notes_app().into(),
            "   ",
            create_note_action(),
        );

        assert_eq!(result.unwrap_err(), CommandContributionError::EmptyTitle);
    }

    #[test]
    fn rejects_title_longer_than_limit() {
        let result = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            notes_app().into(),
            "a".repeat(121),
            create_note_action(),
        );

        assert_eq!(result.unwrap_err(), CommandContributionError::TitleTooLong);
    }

    #[test]
    fn user_cannot_register_command_contribution() {
        let user = UserIdentity::new(UserId::new());

        let result = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            user.into(),
            "New note",
            create_note_action(),
        );

        assert_eq!(
            result.unwrap_err(),
            CommandContributionError::InvalidContribution(ContributionError::UserCannotContribute)
        );
    }

    #[test]
    fn command_can_invoke_capability() {
        let capability = CapabilityId::parse("com.rumahl.notes.create-note").unwrap();
        let command = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            notes_app().into(),
            "New note",
            CommandAction::invoke_capability(capability.clone()),
        )
        .unwrap();
        assert_eq!(command.action().capability(), Some(&capability));
        assert_eq!(command.action().app_id(), None);
    }

    #[test]
    fn command_can_open_app() {
        let app_id = AppId::parse("com.rumahl.notes").unwrap();
        let command = CommandContribution::new(
            ContributionId::parse("com.rumahl.notes.open").unwrap(),
            notes_app().into(),
            "Open Notes",
            CommandAction::open_app(app_id.clone()),
        )
        .unwrap();
        assert_eq!(command.action().app_id(), Some(&app_id));
        assert_eq!(command.action().capability(), None);
    }
}
