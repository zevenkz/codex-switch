import { useRef, type KeyboardEvent } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";

export const LANGUAGE_OPTIONS = ["zh", "en", "ja"] as const;

export type LanguageOption = (typeof LANGUAGE_OPTIONS)[number];

const LANGUAGE_LABEL_KEYS: Record<LanguageOption, string> = {
  zh: "settings.languageOptionChinese",
  en: "settings.languageOptionEnglish",
  ja: "settings.languageOptionJapanese",
};

interface LanguageSettingsProps {
  value: LanguageOption;
  onChange: (value: LanguageOption) => void | Promise<boolean | void>;
}

export function LanguageSettings({ value, onChange }: LanguageSettingsProps) {
  const { t } = useTranslation();
  const optionRefs = useRef<
    Partial<Record<LanguageOption, HTMLButtonElement | null>>
  >({});

  const focusOption = (option: LanguageOption) => {
    optionRefs.current[option]?.focus();
  };

  const selectOption = async (option: LanguageOption) => {
    if (option !== value) {
      const result = await onChange(option);
      return result !== false;
    }

    return true;
  };

  const handleKeyDown = async (
    event: KeyboardEvent<HTMLButtonElement>,
    option: LanguageOption,
  ) => {
    const currentIndex = LANGUAGE_OPTIONS.indexOf(option);

    if (currentIndex === -1) {
      return;
    }

    let nextOption: LanguageOption | null = null;

    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        nextOption =
          LANGUAGE_OPTIONS[(currentIndex + 1) % LANGUAGE_OPTIONS.length];
        break;
      case "ArrowLeft":
      case "ArrowUp":
        nextOption =
          LANGUAGE_OPTIONS[
            (currentIndex - 1 + LANGUAGE_OPTIONS.length) %
              LANGUAGE_OPTIONS.length
          ];
        break;
      case "Home":
        nextOption = LANGUAGE_OPTIONS[0];
        break;
      case "End":
        nextOption = LANGUAGE_OPTIONS[LANGUAGE_OPTIONS.length - 1];
        break;
      default:
        return;
    }

    event.preventDefault();
    const changed = await selectOption(nextOption);
    if (changed) {
      focusOption(nextOption);
    }
  };

  return (
    <section className="space-y-2">
      <header className="space-y-1">
        <h3 className="text-sm font-medium">{t("settings.language")}</h3>
        <p className="text-xs text-muted-foreground">
          {t("settings.languageHint")}
        </p>
      </header>
      <div
        role="radiogroup"
        aria-label={t("settings.language")}
        aria-orientation="horizontal"
        className="inline-flex gap-1 rounded-md border border-border-default bg-background p-1"
      >
        {LANGUAGE_OPTIONS.map((option) => (
          <LanguageButton
            key={option}
            active={value === option}
            buttonRef={(element) => {
              optionRefs.current[option] = element;
            }}
            onClick={() => {
              void selectOption(option);
            }}
            onKeyDown={(event) => handleKeyDown(event, option)}
          >
            {t(LANGUAGE_LABEL_KEYS[option])}
          </LanguageButton>
        ))}
      </div>
    </section>
  );
}

interface LanguageButtonProps {
  active: boolean;
  buttonRef: (element: HTMLButtonElement | null) => void;
  onClick: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  children: React.ReactNode;
}

function LanguageButton({
  active,
  buttonRef,
  onClick,
  onKeyDown,
  children,
}: LanguageButtonProps) {
  return (
    <Button
      ref={buttonRef}
      type="button"
      role="radio"
      onClick={onClick}
      onKeyDown={onKeyDown}
      aria-checked={active}
      tabIndex={active ? 0 : -1}
      size="sm"
      variant={active ? "default" : "ghost"}
      className={cn(
        "min-w-[96px]",
        active
          ? "shadow-sm"
          : "text-muted-foreground hover:text-foreground hover:bg-muted",
      )}
    >
      {children}
    </Button>
  );
}
