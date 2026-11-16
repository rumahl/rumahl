use crate::{ContributionId, RuntimeEntrypointId};

/// Where an installed app's declarative surface is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSlot {
    Desktop,
    Launcher,
    Both,
}

impl SurfaceSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Launcher => "launcher",
            Self::Both => "both",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "desktop" => Some(Self::Desktop),
            "launcher" => Some(Self::Launcher),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
}

/// How the shell renders the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Companion,
    Badge,
    Panel,
}

impl SurfaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Companion => "companion",
            Self::Badge => "badge",
            Self::Panel => "panel",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "companion" => Some(Self::Companion),
            "badge" => Some(Self::Badge),
            "panel" => Some(Self::Panel),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSize {
    Small,
    Medium,
    Large,
}

impl SurfaceSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfacePlacement {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SurfacePlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "top-left" => Some(Self::TopLeft),
            "top-right" => Some(Self::TopRight),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom-right" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceTone {
    Accent,
    Neutral,
    Warm,
    Cool,
    Rose,
    Mint,
}

impl SurfaceTone {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accent => "accent",
            Self::Neutral => "neutral",
            Self::Warm => "warm",
            Self::Cool => "cool",
            Self::Rose => "rose",
            Self::Mint => "mint",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "accent" => Some(Self::Accent),
            "neutral" => Some(Self::Neutral),
            "warm" => Some(Self::Warm),
            "cool" => Some(Self::Cool),
            "rose" => Some(Self::Rose),
            "mint" => Some(Self::Mint),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceIcon {
    Dot,
    Star,
    Bolt,
    Leaf,
    Moon,
    Heart,
    Orbit,
    Pulse,
}

impl SurfaceIcon {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Star => "star",
            Self::Bolt => "bolt",
            Self::Leaf => "leaf",
            Self::Moon => "moon",
            Self::Heart => "heart",
            Self::Orbit => "orbit",
            Self::Pulse => "pulse",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dot" => Some(Self::Dot),
            "star" => Some(Self::Star),
            "bolt" => Some(Self::Bolt),
            "leaf" => Some(Self::Leaf),
            "moon" => Some(Self::Moon),
            "heart" => Some(Self::Heart),
            "orbit" => Some(Self::Orbit),
            "pulse" => Some(Self::Pulse),
            _ => None,
        }
    }
}

/// A signed, data-only extension surface declared by an installed app.
///
/// The shell owns the rendering and interaction. Apps never provide markup,
/// styles or code through this declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceDeclaration {
    id: ContributionId,
    slot: SurfaceSlot,
    kind: SurfaceKind,
    size: SurfaceSize,
    placement: SurfacePlacement,
    tone: SurfaceTone,
    icon: SurfaceIcon,
    entrypoint: Option<RuntimeEntrypointId>,
}

impl SurfaceDeclaration {
    pub fn new(
        id: ContributionId,
        slot: SurfaceSlot,
        kind: SurfaceKind,
        size: SurfaceSize,
        placement: SurfacePlacement,
        tone: SurfaceTone,
        icon: SurfaceIcon,
    ) -> Self {
        Self {
            id,
            slot,
            kind,
            size,
            placement,
            tone,
            icon,
            entrypoint: None,
        }
    }

    /// Tier B: the app draws its own surface in an isolated frame.
    pub fn with_entrypoint(mut self, entrypoint: Option<RuntimeEntrypointId>) -> Self {
        self.entrypoint = entrypoint;
        self
    }

    pub fn entrypoint(&self) -> Option<&RuntimeEntrypointId> {
        self.entrypoint.as_ref()
    }

    pub fn id(&self) -> &ContributionId {
        &self.id
    }

    pub fn slot(&self) -> SurfaceSlot {
        self.slot
    }

    pub fn kind(&self) -> SurfaceKind {
        self.kind
    }

    pub fn size(&self) -> SurfaceSize {
        self.size
    }

    pub fn placement(&self) -> SurfacePlacement {
        self.placement
    }

    pub fn tone(&self) -> SurfaceTone {
        self.tone
    }

    pub fn icon(&self) -> SurfaceIcon {
        self.icon
    }
}
