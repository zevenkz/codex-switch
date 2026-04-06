import { useEffect, useState } from "react";
import { toast } from "sonner";
import { mockAccounts } from "../mockAccounts";
import type { CodexAccount, CodexQuotaWindow } from "../types";
import {
  codexAccountsApi,
  type CodexAccountRecord,
  type CodexAccountsUpdatedEventPayload,
  type CodexQuotaSnapshot,
} from "../api";

type SyncOptions = {
  fallbackToPreview: boolean;
};

type RefreshOptions = {
  force?: boolean;
  silent: boolean;
};

function formatQuotaTime(epochSeconds: number | null): string {
  if (epochSeconds == null) {
    return "—";
  }

  const date = new Date(epochSeconds * 1000);
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

function formatQuotaDate(epochSeconds: number | null): string {
  if (epochSeconds == null) {
    return "—";
  }

  const date = new Date(epochSeconds * 1000);
  const parts = new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
  }).formatToParts(date);
  const month = parts.find((part) => part.type === "month")?.value ?? "";
  const day = parts.find((part) => part.type === "day")?.value ?? "";
  return `${month}月${day}日`;
}

function planFromRecord(planType: string | null): CodexAccount["plan"] {
  const normalized = planType?.trim().toLowerCase();
  if (normalized === "enterprise") {
    return "Enterprise";
  }
  if (normalized === "team") {
    return "Team";
  }
  return "Plus";
}

function avatarSeedFromRecord(record: CodexAccountRecord): string {
  const rawSeed = record.avatar_seed?.trim();
  if (rawSeed) {
    return rawSeed.slice(0, 1).toUpperCase();
  }

  const source = record.display_name ?? record.email ?? record.id;
  return source.slice(0, 1).toUpperCase();
}

function quotaWindowsFromSnapshot(quota: CodexQuotaSnapshot | null): ReadonlyArray<CodexQuotaWindow> {
  if (!quota) {
    return [];
  }

  const windows: CodexQuotaWindow[] = [];
  if (quota.five_hour_percent !== null) {
    windows.push({
      id: "five-hours",
      label: "fiveHours",
      remainingPercent: quota.five_hour_percent,
      resetAt: formatQuotaTime(quota.five_hour_reset_at),
    });
  }
  if (quota.week_percent !== null) {
    windows.push({
      id: "week",
      label: "week",
      remainingPercent: quota.week_percent,
      resetAt: formatQuotaDate(quota.week_reset_at),
    });
  }

  return windows;
}

function isExpiredQuotaError(value: string | null): boolean {
  if (!value) {
    return false;
  }

  const normalized = value.toLowerCase();
  return (
    normalized.includes("expired") ||
    normalized.includes("stale") ||
    normalized.includes("re-login") ||
    normalized.includes("relogin") ||
    normalized.includes("oauth")
  );
}

function accountFromRecord(record: CodexAccountRecord): CodexAccount {
  const email = record.email ?? record.display_name ?? record.id;
  const quotaError = record.quota?.last_error ?? null;
  return {
    id: record.id,
    email,
    plan: planFromRecord(record.plan_type),
    status: record.is_active
      ? "active"
      : isExpiredQuotaError(quotaError)
        ? "needs_login"
        : "available",
    avatarSeed: avatarSeedFromRecord(record),
    quotas: quotaWindowsFromSnapshot(record.quota),
    quotaError,
  };
}

function upsertAccount(
  currentAccounts: ReadonlyArray<CodexAccount>,
  nextAccount: CodexAccount,
): ReadonlyArray<CodexAccount> {
  const existingIndex = currentAccounts.findIndex(
    (account) => account.id === nextAccount.id,
  );

  if (existingIndex === -1) {
    return [...currentAccounts, nextAccount];
  }

  return currentAccounts.map((account, index) =>
    index === existingIndex ? nextAccount : account,
  );
}

export function useCodexAccounts() {
  const [accounts, setAccounts] = useState<ReadonlyArray<CodexAccount>>(mockAccounts);
  const [isBackendAvailable, setIsBackendAvailable] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);

  async function syncAccounts(options: SyncOptions): Promise<boolean> {
    try {
      const records = await codexAccountsApi.listAccounts();
      setAccounts(records.map(accountFromRecord));
      setIsBackendAvailable(true);
      return true;
    } catch {
      setIsBackendAvailable(false);
      if (options.fallbackToPreview) {
        setAccounts(mockAccounts);
      }
      return false;
    }
  }

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    const load = async () => {
      if (disposed) {
        return;
      }

      const backendLoaded = await syncAccounts({ fallbackToPreview: true });
      if (!disposed && backendLoaded) {
        await refreshAllInternal({ force: true, silent: true });
      }
    };

    void load();
    void codexAccountsApi.onAccountsUpdated((payload?: CodexAccountsUpdatedEventPayload) => {
      if (!disposed) {
        if (payload?.error) {
          toast.error(`刷新 Codex 额度失败: ${payload.error}`);
        }
        void syncAccounts({ fallbackToPreview: false });
      }
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function addAccount(): Promise<void> {
    let authorizeUrlOpened = false;
    try {
      const session = await codexAccountsApi.startOAuth();
      await codexAccountsApi.openAuthorizeUrl(session.authorize_url);
      authorizeUrlOpened = true;
      const savedAccount = await codexAccountsApi.completeOAuth();
      setAccounts((currentAccounts) =>
        upsertAccount(currentAccounts, accountFromRecord(savedAccount)),
      );
      setIsBackendAvailable(true);
      try {
        await syncAccounts({ fallbackToPreview: false });
      } catch (syncError) {
        console.warn(
          "[useCodexAccounts] Account saved but account resync failed",
          syncError,
        );
      }
      return;
    } catch (error) {
      if (!authorizeUrlOpened) {
        await codexAccountsApi.openFallbackLoginPage();
        return;
      }

      console.error("[useCodexAccounts] Failed to add Codex account", error);
      toast.error("新增账号失败");
    }
  }

  async function reloginAccount(): Promise<void> {
    let authorizeUrlOpened = false;
    try {
      const session = await codexAccountsApi.startOAuth();
      await codexAccountsApi.openAuthorizeUrl(session.authorize_url);
      authorizeUrlOpened = true;
      const savedAccount = await codexAccountsApi.completeOAuth();
      setAccounts((currentAccounts) =>
        upsertAccount(currentAccounts, accountFromRecord(savedAccount)),
      );
      setIsBackendAvailable(true);
      await codexAccountsApi.switchAccount(savedAccount.id);
      await syncAccounts({ fallbackToPreview: false });
      toast.success("账号已更新并切换成功");
    } catch (error) {
      if (!authorizeUrlOpened) {
        await codexAccountsApi.openFallbackLoginPage();
        return;
      }

      console.error("[useCodexAccounts] Failed to re-login Codex account", error);
      toast.error("重新登录未完成");
      await syncAccounts({ fallbackToPreview: false });
    }
  }

  async function refreshAllInternal(options: RefreshOptions): Promise<void> {
    if (!options.force && !isBackendAvailable) {
      return;
    }

    setIsRefreshing(true);
    try {
      await codexAccountsApi.refreshAllAccountQuotas();
    } catch (error) {
      if (!options.silent) {
        const message =
          error instanceof Error ? error.message : String(error ?? "unknown error");
        toast.error(`刷新 Codex 额度失败: ${message}`);
      }
    } finally {
      await syncAccounts({ fallbackToPreview: false });
      setIsRefreshing(false);
    }
  }

  async function refreshAll(): Promise<void> {
    await refreshAllInternal({ silent: false });
  }

  async function switchAccount(accountId: string): Promise<void> {
    const targetAccount = accounts.find((account) => account.id === accountId);
    if (targetAccount?.status === "needs_login") {
      await reloginAccount();
      return;
    }

    if (!isBackendAvailable) {
      setAccounts((currentAccounts) =>
        currentAccounts.map((account) => ({
          ...account,
          status:
            account.id === accountId ? "active" : account.status === "active" ? "available" : account.status,
        })),
      );
      return;
    }

    await codexAccountsApi.switchAccount(accountId);
    await syncAccounts({ fallbackToPreview: false });
  }

  async function deleteAccount(accountId: string): Promise<void> {
    if (!isBackendAvailable) {
      setAccounts((currentAccounts) =>
        currentAccounts.filter((account) => account.id !== accountId),
      );
      return;
    }

    await codexAccountsApi.deleteAccount(accountId);
    await syncAccounts({ fallbackToPreview: false });
  }

  return {
    accounts,
    isRefreshing,
    addAccount,
    refreshAll,
    switchAccount,
    deleteAccount,
  };
}
