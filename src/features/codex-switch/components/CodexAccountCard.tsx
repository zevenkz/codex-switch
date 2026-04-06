import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { CodexAccount } from "../types";

interface CodexAccountCardProps {
  account: CodexAccount;
  onSwitch?: (account: CodexAccount) => void;
  onDelete?: (account: CodexAccount) => void;
}

export function CodexAccountCard({
  account,
  onSwitch,
  onDelete,
}: CodexAccountCardProps) {
  const { t } = useTranslation();
  const quotas = account.quotas;

  return (
    <article className="rounded-[26px] border border-black/[0.06] bg-black/[0.02] px-5 py-4 transition-colors hover:bg-black/[0.035] dark:border-white/[0.08] dark:bg-[#2a3040] dark:hover:bg-[#303748]">
      <div className="flex flex-col gap-4 md:grid md:grid-cols-[minmax(0,1fr)_200px_132px] md:items-center md:gap-4">
        <div className="flex min-w-0 flex-1 items-center gap-4">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-[18px] bg-blue-500 text-sm font-semibold text-white">
            {account.avatarSeed}
          </div>
          <div className="min-w-0">
            <h2 className="truncate text-[17px] font-semibold text-slate-900 dark:text-slate-100">
              {account.email}
            </h2>
            <div className="mt-1 flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
              <p>
              {t("codexSwitch.accounts.plan", { plan: account.plan })}
              </p>
              {account.status === "needs_login" ? (
                <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[12px] font-medium text-amber-600 dark:bg-amber-400/10 dark:text-amber-300">
                  {t("codexSwitch.accounts.invalid")}
                </span>
              ) : null}
            </div>
          </div>
        </div>

        <div
          data-testid={`quota-panel-${account.id}`}
          className="flex min-w-0 justify-start md:-ml-24 md:w-[200px] md:justify-self-start"
        >
          <div className="w-full">
            {quotas.length > 0 ? (
              <div className="space-y-1">
                {quotas.map((quota) => (
                  <div
                    key={quota.id}
                    className="grid grid-cols-[42px_minmax(0,1fr)] items-center gap-2"
                  >
                    <span className="text-[13px] font-semibold text-slate-700 dark:text-slate-300">
                      {t(`codexSwitch.quota.${quota.label}`)}
                    </span>
                    <span className="flex items-center justify-end gap-1.5 whitespace-nowrap text-right text-[13px] tabular-nums text-slate-400 dark:text-slate-500">
                      <span>{quota.remainingPercent}%</span>
                      <span className="whitespace-nowrap">{quota.resetAt}</span>
                    </span>
                  </div>
                ))}
              </div>
            ) : account.quotaError ? (
              <p
                title={account.quotaError}
                className="truncate whitespace-nowrap text-[13px] text-amber-500/80 dark:text-amber-300/80"
              >
                {t("codexSwitch.quota.refreshFailed")}
              </p>
            ) : null}
          </div>
        </div>

        <div
          data-testid={`account-actions-${account.id}`}
          className="flex items-center justify-end gap-3 md:w-[132px] md:justify-self-end"
        >
          {account.status === "active" ? (
            <Button
              type="button"
              size="sm"
              disabled
              className="rounded-full bg-blue-500/12 px-4 text-blue-700 hover:bg-blue-500/12 dark:bg-blue-400/10 dark:text-blue-200"
            >
              {t("codexSwitch.accounts.inUse")}
            </Button>
          ) : (
            <Button
              type="button"
              size="sm"
              onClick={() => onSwitch?.(account)}
              className="rounded-full border border-black/[0.08] bg-transparent px-4 text-slate-700 hover:bg-black/[0.05] dark:border-white/[0.1] dark:text-slate-200 dark:hover:bg-white/[0.05]"
            >
              {account.status === "needs_login"
                ? t("codexSwitch.accounts.relogin")
                : t("codexSwitch.accounts.enable")}
            </Button>
          )}
          {account.status !== "active" ? (
            <Button
              type="button"
              size="icon"
              variant="ghost"
              aria-label={`${t("common.delete")} ${account.email}`}
              onClick={() => onDelete?.(account)}
              className="h-10 w-10 rounded-2xl text-slate-500 hover:bg-red-500/10 hover:text-red-500 dark:text-slate-400 dark:hover:bg-red-500/10 dark:hover:text-red-300"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          ) : null}
        </div>
      </div>
    </article>
  );
}
