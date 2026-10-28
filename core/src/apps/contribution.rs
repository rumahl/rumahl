use std::error::Error;
use std::fmt;

use crate::{CapabilityId, CommandAction, ContributionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionDeclaration {
    Command(CommandContributionDeclaration),
    Search(SearchContributionDeclaration),
}

impl ContributionDeclaration {
    pub fn id(&self) -> &ContributionId {
        match self {
            Self::Command(command) => command.id(),
            Self::Search(search) => search.id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContributionDeclaration {
    id: ContributionId,
    title: String,
    action: CommandAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandContributionDeclarationError {
    EmptyTitle,
    TitleTooLong,
}

impl CommandContributionDeclaration {
    pub fn new(
        id: ContributionId,
        title: impl Into<String>,
        action: CommandAction,
    ) -> Result<Self, CommandContributionDeclarationError> {
        let title = title.into();
        let title = title.trim();

        if title.is_empty() {
            return Err(CommandContributionDeclarationError::EmptyTitle);
        }

        if title.chars().count() > 120 {
            return Err(CommandContributionDeclarationError::TitleTooLong);
        }

        Ok(Self {
            id,
            title: title.to_owned(),
            action,
        })
    }

    pub fn id(&self) -> &ContributionId {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn action(&self) -> &CommandAction {
        &self.action
    }
}

impl fmt::Display for CommandContributionDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => {
                write!(f, "command contribution title cannot be empty")
            }

            Self::TitleTooLong => {
                write!(f, "command contribution title cannot exceed 120 characters")
            }
        }
    }
}

impl Error for CommandContributionDeclarationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchContributionDeclaration {
    id: ContributionId,
    capability: CapabilityId,
}

impl SearchContributionDeclaration {
    pub fn new(id: ContributionId, capability: CapabilityId) -> Self {
        Self { id, capability }
    }

    pub fn id(&self) -> &ContributionId {
        &self.id
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }
}

impl From<CommandContributionDeclaration> for ContributionDeclaration {
    fn from(contribution: CommandContributionDeclaration) -> Self {
        Self::Command(contribution)
    }
}

impl From<SearchContributionDeclaration> for ContributionDeclaration {
    fn from(contribution: SearchContributionDeclaration) -> Self {
        Self::Search(contribution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::AppId;

    #[test]
    fn creates_command_contribution_declaration() {
        let capability = CapabilityId::parse("com.rumahl.notes.create-note").unwrap();

        let declaration = CommandContributionDeclaration::new(
            ContributionId::parse("com.rumahl.notes.new-note").unwrap(),
            "New note",
            CommandAction::invoke_capability(capability.clone()),
        )
        .unwrap();

        assert_eq!(declaration.id().as_str(), "com.rumahl.notes.new-note");

        assert_eq!(declaration.title(), "New note");

        assert_eq!(declaration.action().capability(), Some(&capability));
    }

    #[test]
    fn command_declaration_trims_title() {
        let declaration = CommandContributionDeclaration::new(
            ContributionId::parse("com.rumahl.notes.open").unwrap(),
            "   Open Notes   ",
            CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
        )
        .unwrap();

        assert_eq!(declaration.title(), "Open Notes");
    }

    #[test]
    fn rejects_empty_command_title() {
        let result = CommandContributionDeclaration::new(
            ContributionId::parse("com.rumahl.notes.open").unwrap(),
            "   ",
            CommandAction::open_app(AppId::parse("com.rumahl.notes").unwrap()),
        );

        assert_eq!(
            result.unwrap_err(),
            CommandContributionDeclarationError::EmptyTitle
        );
    }

    #[test]
    fn creates_search_contribution_declaration() {
        let capability = CapabilityId::parse("rumahl.search.query").unwrap();

        let declaration = SearchContributionDeclaration::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            capability.clone(),
        );

        assert_eq!(declaration.id().as_str(), "com.rumahl.notes.search");

        assert_eq!(declaration.capability(), &capability);
    }

    #[test]
    fn contribution_declaration_exposes_id() {
        let declaration: ContributionDeclaration = SearchContributionDeclaration::new(
            ContributionId::parse("com.rumahl.notes.search").unwrap(),
            CapabilityId::parse("rumahl.search.query").unwrap(),
        )
        .into();

        assert_eq!(declaration.id().as_str(), "com.rumahl.notes.search");
    }
}
