export type CodexAccountStatus = "active" | "available" | "needs_login";

export interface CodexQuotaWindow {
  id: string;
  label: "fiveHours" | "week";
  remainingPercent: number;
  resetAt: string;
}

export interface CodexAccount {
  id: string;
  email: string;
  plan: "Plus" | "Team" | "Enterprise";
  status: CodexAccountStatus;
  avatarSeed: string;
  quotas: ReadonlyArray<CodexQuotaWindow>;
  quotaError: string | null;
}
