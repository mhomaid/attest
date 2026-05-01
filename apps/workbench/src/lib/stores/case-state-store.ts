import { create } from "zustand";

type Disposition = "none" | "approved" | "override";

type CaseEntry = {
  disposition: Disposition;
  overrideLabel?: string;
  updatedAt: number;
};

type CaseStateStore = {
  cases: Record<string, CaseEntry>;
  setApproved: (caseId: string) => void;
  setOverride: (caseId: string, label: string) => void;
  clear: (caseId: string) => void;
};

export const useCaseStateStore = create<CaseStateStore>((set) => ({
  cases: {},
  setApproved: (caseId) =>
    set((s) => ({
      cases: {
        ...s.cases,
        [caseId]: { disposition: "approved", updatedAt: Date.now() },
      },
    })),
  setOverride: (caseId, label) =>
    set((s) => ({
      cases: {
        ...s.cases,
        [caseId]: { disposition: "override", overrideLabel: label, updatedAt: Date.now() },
      },
    })),
  clear: (caseId) =>
    set((s) => {
      const next = { ...s.cases };
      delete next[caseId];
      return { cases: next };
    }),
}));
