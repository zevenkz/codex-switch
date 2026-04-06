import type { CodexAccount } from "./types";

export const mockAccounts: ReadonlyArray<CodexAccount> = [
  {
    id: "acct-1",
    email: "amy@openai.dev",
    plan: "Plus",
    status: "active",
    avatarSeed: "A",
    quotaError: null,
    quotas: [
      {
        id: "five-hours",
        label: "fiveHours",
        remainingPercent: 35,
        resetAt: "15:27",
      },
      {
        id: "week",
        label: "week",
        remainingPercent: 38,
        resetAt: "4月12日",
      },
    ],
  },
  {
    id: "acct-2",
    email: "research@openai.dev",
    plan: "Enterprise",
    status: "available",
    avatarSeed: "R",
    quotaError: null,
    quotas: [
      {
        id: "five-hours",
        label: "fiveHours",
        remainingPercent: 62,
        resetAt: "18:05",
      },
      {
        id: "week",
        label: "week",
        remainingPercent: 71,
        resetAt: "4月15日",
      },
    ],
  },
];
