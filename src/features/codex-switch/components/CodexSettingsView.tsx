import {
  LANGUAGE_OPTIONS,
  LanguageSettings,
  type LanguageOption,
} from "@/components/settings/LanguageSettings";
import { ThemeSettings } from "@/components/settings/ThemeSettings";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";

export function resolveSupportedLanguage(language: string): LanguageOption {
  const normalized = language.toLowerCase();

  if (LANGUAGE_OPTIONS.some((option) => option === normalized)) {
    return normalized as LanguageOption;
  }

  if (normalized.startsWith("en")) {
    return "en";
  }

  if (normalized.startsWith("ja")) {
    return "ja";
  }

  if (normalized.startsWith("zh")) {
    return "zh";
  }

  return "zh";
}

interface CodexSettingsViewProps {
  onBack: () => void;
}

export function CodexSettingsView({ onBack }: CodexSettingsViewProps) {
  const { i18n, t } = useTranslation();
  const currentLanguage = resolveSupportedLanguage(
    i18n.resolvedLanguage ?? i18n.language,
  );

  const handleLanguageChange = async (
    language: LanguageOption,
  ): Promise<boolean> => {
    try {
      await i18n.changeLanguage(language);

      if (typeof window !== "undefined") {
        window.localStorage.setItem("language", language);
      }
      return true;
    } catch (error) {
      console.error("[CodexSettingsView] Failed to change language", error);
      toast.error(t("common.error"));
      return false;
    }
  };

  return (
    <section className="flex flex-1 flex-col gap-10 px-4 py-2 md:px-2">
      <header className="pb-6">
        <div className="flex items-center gap-4">
          <Button
            type="button"
            size="icon"
            variant="outline"
            aria-label={t("common.back")}
            onClick={onBack}
            className="h-11 w-11 rounded-2xl border-white/10 bg-white/[0.03] text-slate-300 shadow-none hover:bg-white/[0.06] dark:border-white/10 dark:bg-white/[0.03] dark:hover:bg-white/[0.06]"
          >
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-slate-50">
            {t("common.settings")}
          </h1>
        </div>
      </header>

      <div className="flex flex-col gap-8">
        <div className="pb-8">
          <LanguageSettings
            value={currentLanguage}
            onChange={handleLanguageChange}
          />
        </div>

        <div>
          <ThemeSettings />
        </div>
      </div>
    </section>
  );
}
