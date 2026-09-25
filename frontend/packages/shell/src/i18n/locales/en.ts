export interface PluralMessage {
  one: string;
  other: string;
}

export type Message = string | PluralMessage;

export const english = {
  "appManager.body": "rumahl OS installs and updates apps safely in the background.",
  "appManager.count": { one: "{count} app", other: "{count} apps" },
  "appManager.ready": "Your apps are ready.",
  "appManager.selectPackage": "Select package",
  "appManager.subtitle": "rumahl System",
  "appManager.title": "App manager",
  "command.available": "Available commands",
  "command.input": "Search commands",
  "command.placeholder": "What would you like to do?",
  "command.title": "Command palette",
  "contribution.count": { one: "{count} contribution", other: "{count} contributions" },
  "contribution.protected": "Protected action",
  "contribution.source": "From your apps",
  "contribution.title": "Quick access",
  "contribution.widget": "Isolated widget slot",
  "contribution.widgetLoading": "Loading widget…",
  "contribution.widgetUnavailable": "Widget unavailable. The rest of the system remains ready.",
  "session.signIn": "Sign in",
  "session.signOut": "Sign out",
  "session.expired": "Your session has ended. Sign in again to continue.",
  "dashboard.greeting": "Good evening, {name}",
  "dashboard.intro":
    "Your local system runs quietly in the background. Apps, data, and identities stay where they belong.",
  "dashboard.title": "Everything at home.",
  "nav.activity": "Activity",
  "nav.apps": "Apps",
  "nav.home": "Overview",
  "nav.main": "Main navigation",
  "nav.settings": "Settings",
  "nav.status": "System ready",
  "profile.open": "Open user menu",
  "search.system": "Search system",
  "section.activity.body":
    "System events will appear here without secrets or sensitive user data.",
  "section.activity.eyebrow": "Traceable",
  "section.activity.title": "Activity",
  "section.apps.body": "Install, update, and remove with resumable, journaled operations.",
  "section.apps.eyebrow": "Local applications",
  "section.apps.title": "Apps",
  "section.settings.body":
    "User, theme, and device settings stay separate from application content.",
  "section.settings.eyebrow": "Your system",
  "section.settings.title": "Settings",
  "status.activity.label": "Latest activity",
  "status.activity.none": "No activity yet",
  "status.apps.label": "Installed apps",
  "status.apps.open": "Open app manager",
  "status.apps.value": { one: "{count} local app", other: "{count} local apps" },
  "status.protection.attention": "Needs attention",
  "status.protection.active": "Active",
  "status.protection.label": "System protection",
  "status.protection.value": "Everything secure",
  "status.title": "System status",
  "stream.available": "Running graphical apps",
  "stream.connecting": "Connecting to the app…",
  "stream.none": "No graphical apps are running.",
  "stream.subtitle": "Graphical app",
  "stream.unavailable": "The stream is unavailable. Other system functions remain ready.",
  "window.close": "Close {title}",
  "window.controls": "Window controls",
  "window.minimize": "Minimize {title}",
  "window.minimized": "Minimized windows"
} as const;

export type MessageKey = keyof typeof english;
export type Messages = Record<MessageKey, Message>;
