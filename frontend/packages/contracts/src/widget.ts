export interface WidgetFrameDescriptorV1 {
  frameVersion: 1;
  contributionId: string;
  frameUrl: string;
}

export function parseWidgetFrameDescriptor(
  value: unknown,
  contributionId: string,
  shellOrigin: string,
  allowDevelopmentLoopback = false
): WidgetFrameDescriptorV1 {
  if (!isRecord(value) || Object.keys(value).length !== 3 ||
      value.frameVersion !== 1 || value.contributionId !== contributionId ||
      typeof value.frameUrl !== "string" || value.frameUrl.length > 2048) {
    throw new Error("invalid widget frame descriptor");
  }
  let frameUrl: URL;
  try {
    frameUrl = new URL(value.frameUrl);
  } catch {
    throw new Error("invalid widget frame URL");
  }
  const secure = frameUrl.protocol === "https:";
  const developmentLoopback = allowDevelopmentLoopback && frameUrl.protocol === "http:" &&
    frameUrl.hostname === "127.0.0.1";
  const shell = new URL(shellOrigin);
  if ((!secure && !developmentLoopback) || frameUrl.hostname === shell.hostname ||
      frameUrl.username || frameUrl.password || frameUrl.search || frameUrl.hash) {
    throw new Error("unsafe widget frame URL");
  }
  return value as unknown as WidgetFrameDescriptorV1;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
