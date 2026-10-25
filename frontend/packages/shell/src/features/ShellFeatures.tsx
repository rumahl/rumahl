import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type PropsWithChildren
} from "react";

export type FeatureId =
  | "recovery"
  | "recovery.offline"
  | "recovery.center"
  | "activity"
  | "session.signOut"
  | "app.install";

export type FeatureState = "idle" | "loading" | "ready" | "disabled" | "unavailable";

export interface FeatureInfo {
  state: FeatureState;
  reason?: "demo" | "loading" | "error" | "unavailable";
}

export type FeatureMap = Record<FeatureId, FeatureInfo>;

type FeatureLoader = () => Promise<void>;

interface FeatureContextValue {
  features: FeatureMap;
  ensure: (id: FeatureId) => Promise<FeatureState>;
  register: (id: FeatureId, load: FeatureLoader) => void;
}

const IDS: readonly FeatureId[] = [
  "recovery",
  "recovery.offline",
  "recovery.center",
  "activity",
  "session.signOut",
  "app.install"
];

const DEMO_DISABLED: readonly FeatureId[] = [
  "recovery",
  "recovery.offline",
  "recovery.center",
  "activity",
  "session.signOut",
  "app.install"
];

function initialFeatures(demo: boolean): FeatureMap {
  const map = {} as FeatureMap;
  for (const id of IDS) {
    map[id] = demo && DEMO_DISABLED.includes(id)
      ? { state: "disabled", reason: "demo" }
      : { state: "ready" };
  }
  return map;
}

const DEFAULT_FEATURES = initialFeatures(false);

const Context = createContext<FeatureContextValue>({
  features: DEFAULT_FEATURES,
  ensure: async (id) => DEFAULT_FEATURES[id].state,
  register: () => {}
});

export function ShellFeatureProvider({ demo = false, children }: PropsWithChildren<{ demo?: boolean | undefined }>) {
  const [features, setFeatures] = useState<FeatureMap>(() => initialFeatures(demo));
  const current = useRef(features);
  current.current = features;
  const loaders = useRef(new Map<FeatureId, FeatureLoader>());

  const setOne = useCallback((id: FeatureId, info: FeatureInfo) => {
    setFeatures((previous) => ({ ...previous, [id]: info }));
  }, []);

  const ensure = useCallback(async (id: FeatureId): Promise<FeatureState> => {
    const state = current.current[id];
    if (state.state !== "idle") return state.state;
    const load = loaders.current.get(id);
    if (!load) {
      setOne(id, { state: "ready" });
      return "ready";
    }
    setOne(id, { state: "loading", reason: "loading" });
    try {
      await load();
      setOne(id, { state: "ready" });
      return "ready";
    } catch {
      setOne(id, { state: "unavailable", reason: "error" });
      return "unavailable";
    }
  }, [setOne]);

  const register = useCallback((id: FeatureId, load: FeatureLoader) => {
    loaders.current.set(id, load);
  }, []);

  const value = useMemo<FeatureContextValue>(
    () => ({ features, ensure, register }),
    [features, ensure, register]
  );
  return <Context value={value}>{children}</Context>;
}

export function useFeatures(): FeatureMap {
  return useContext(Context).features;
}

export function useFeature(id: FeatureId): FeatureInfo {
  return useContext(Context).features[id];
}

export function useEnsureFeature(): (id: FeatureId) => Promise<FeatureState> {
  return useContext(Context).ensure;
}

export function useRegisterFeature(): (id: FeatureId, load: FeatureLoader) => void {
  return useContext(Context).register;
}

export function featureReady(info: FeatureInfo | undefined): boolean {
  return info?.state === "ready";
}
