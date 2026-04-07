import { useState } from "react";
import { Plus, RefreshCw, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useTranslation } from "react-i18next";
import type { CodexAccount } from "../types";
import { CodexAccountCard } from "./CodexAccountCard";

interface CodexAccountsViewProps {
  accounts: ReadonlyArray<CodexAccount>;
  isRefreshing: boolean;
  onAddAccount: () => void;
  onRefreshQuota: () => void;
  onOpenSettings: () => void;
  onSwitchAccount: (accountId: string) => void;
  onDeleteAccount: (accountId: string) => void;
}

export function CodexAccountsView({
  accounts,
  isRefreshing,
  onAddAccount,
  onRefreshQuota,
  onOpenSettings,
  onSwitchAccount,
  onDeleteAccount,
}: CodexAccountsViewProps) {
  const { t } = useTranslation();
  const [accountPendingDeletion, setAccountPendingDeletion] =
    useState<CodexAccount | null>(null);

  const handleSwitchAccount = (accountId: string) => {
    setAccountPendingDeletion(null);
    onSwitchAccount(accountId);
  };

  return (
    <section className="flex flex-1 flex-col gap-8">
      <header className="flex flex-wrap items-center justify-between gap-6 px-2 pt-2">
        <div className="flex items-center gap-3">
          <h1 className="text-[34px] font-semibold tracking-tight text-blue-500 dark:text-blue-400">
              Codex Switch
          </h1>
          <Button
            type="button"
            size="icon"
            variant="ghost"
            aria-label={t("common.settings")}
            onClick={onOpenSettings}
            className="h-10 w-10 rounded-xl text-slate-500 hover:bg-black/5 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-white/8 dark:hover:text-slate-100"
          >
            <Settings className="h-5 w-5" />
          </Button>
        </div>
        <div className="flex items-center gap-3">
          <Button
            type="button"
            size="icon"
            variant="ghost"
            aria-label={t("codexSwitch.accounts.refreshQuota")}
            onClick={onRefreshQuota}
            disabled={isRefreshing}
            className="h-11 w-11 rounded-2xl bg-black/[0.04] text-slate-500 hover:bg-black/[0.07] hover:text-slate-900 dark:bg-white/[0.04] dark:text-slate-400 dark:hover:bg-white/[0.08] dark:hover:text-slate-100"
          >
            <RefreshCw className={`h-4 w-4 ${isRefreshing ? "animate-spin" : ""}`} />
          </Button>
          <Button
            type="button"
            size="icon"
            aria-label={t("codexSwitch.accounts.addAccount")}
            onClick={onAddAccount}
            className="h-12 w-12 rounded-full bg-orange-500 text-white shadow-[0_10px_24px_rgba(249,115,22,0.32)] hover:bg-orange-400"
          >
            <Plus className="h-5 w-5" />
          </Button>
        </div>
      </header>

      <div className="px-2">
        <p className="text-sm font-medium text-slate-500 dark:text-slate-400">
          {t("codexSwitch.accounts.countSummary", { count: accounts.length })}
        </p>
      </div>

      <div className="flex flex-col gap-3">
        {accounts.map((account) => (
          <CodexAccountCard
            key={account.id}
            account={account}
            onSwitch={
              account.status === "active"
                ? undefined
                : () => handleSwitchAccount(account.id)
            }
            onDelete={
              account.status === "active"
                ? undefined
                : () => setAccountPendingDeletion(account)
            }
          />
        ))}
      </div>

      <ConfirmDialog
        isOpen={accountPendingDeletion !== null}
        title={t("codexSwitch.accounts.deleteTitle")}
        message={t("codexSwitch.accounts.deleteMessage", {
          email: accountPendingDeletion?.email ?? "",
        })}
        confirmText={t("codexSwitch.accounts.deleteConfirm")}
        onConfirm={() => {
          if (accountPendingDeletion) {
            onDeleteAccount(accountPendingDeletion.id);
          }
          setAccountPendingDeletion(null);
        }}
        onCancel={() => setAccountPendingDeletion(null)}
      />
    </section>
  );
}
