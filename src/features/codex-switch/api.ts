import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { settingsApi } from "@/lib/api/settings";

export interface CodexQuotaSnapshot {
  five_hour_percent: number | null;
  five_hour_reset_at: number | null;
  week_percent: number | null;
  week_reset_at: number | null;
  refreshed_at: number;
  last_error: string | null;
}

export interface CodexAccountRecord {
  id: string;
  email: string | null;
  account_id: string | null;
  plan_type: string | null;
  display_name: string | null;
  avatar_seed: string;
  added_at: number;
  last_used_at: number | null;
  is_active: boolean;
  auth_json: Record<string, unknown>;
  quota: CodexQuotaSnapshot | null;
  metadata: Record<string, unknown>;
}

export interface PendingCodexAccountOAuthSession {
  state: string;
  authorize_url: string;
  callback_port: number;
}

export interface CodexAccountsUpdatedEventPayload {
  source?: string;
  error?: string;
}

const FALLBACK_LOGIN_URL = "https://auth.openai.com/log-in";

async function invokeWithFallback<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (args === undefined) {
    return await invoke<T>(command);
  }

  return await invoke<T>(command, args);
}

export const codexAccountsApi = {
  async listAccounts(): Promise<CodexAccountRecord[]> {
    return await invokeWithFallback("list_codex_accounts");
  },

  async getActiveAccount(): Promise<CodexAccountRecord | null> {
    return await invokeWithFallback("get_active_codex_account");
  },

  async startOAuth(): Promise<PendingCodexAccountOAuthSession> {
    return await invokeWithFallback("start_codex_account_oauth");
  },

  async cancelOAuth(): Promise<boolean> {
    return await invokeWithFallback("cancel_codex_account_oauth");
  },

  async completeOAuth(): Promise<CodexAccountRecord> {
    return await invokeWithFallback("complete_codex_account_oauth");
  },

  async switchAccount(accountId: string): Promise<boolean> {
    return await invokeWithFallback("switch_codex_account", { accountId });
  },

  async deleteAccount(accountId: string): Promise<boolean> {
    return await invokeWithFallback("delete_codex_account", { accountId });
  },

  async refreshAccountQuota(accountId: string): Promise<CodexAccountRecord> {
    return await invokeWithFallback("refresh_codex_account_quota", {
      accountId,
    });
  },

  async refreshAllAccountQuotas(): Promise<CodexAccountRecord[]> {
    return await invokeWithFallback("refresh_all_codex_account_quotas");
  },

  async openAuthorizeUrl(authorizeUrl: string): Promise<void> {
    try {
      await settingsApi.openExternal(authorizeUrl);
      return;
    } catch {
      // Fall back to a plain browser tab when the Tauri bridge is unavailable.
    }

    if (typeof window !== "undefined" && typeof window.open === "function") {
      window.open(authorizeUrl, "_blank", "noopener,noreferrer");
      return;
    }

    throw new Error(`Unable to open authorize URL: ${authorizeUrl}`);
  },

  async openFallbackLoginPage(): Promise<void> {
    await this.openAuthorizeUrl(FALLBACK_LOGIN_URL);
  },

  async onAccountsUpdated(
    handler: (payload?: CodexAccountsUpdatedEventPayload) => void | Promise<void>,
  ): Promise<UnlistenFn | null> {
    try {
      return await listen<CodexAccountsUpdatedEventPayload>(
        "codex-accounts-updated",
        (event) => {
          void handler(event.payload);
        },
      );
    } catch {
      return null;
    }
  },
};
