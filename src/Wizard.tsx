import { useState, useTransition } from "react";
import { useLanguage, type Language } from "./i18n";
import { initializeLocalDatabase, completeWizard } from "./desktop";
import "./Wizard.css";

type WizardStep = "language" | "database" | "provider" | "completed";

export function Wizard({ onComplete }: { onComplete: () => void }) {
  const { t, language, setLanguage } = useLanguage();
  const [step, setStep] = useState<WizardStep>("language");
  const [dbDone, setDbDone] = useState(false);
  const [dbError, setDbError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  const handleLanguageNext = () => setStep("database");

  const handleInitDb = () => {
    startTransition(async () => {
      try {
        setDbError(null);
        await initializeLocalDatabase();
        setDbDone(true);
      } catch (e) {
        setDbError(e instanceof Error ? e.message : String(e));
      }
    });
  };

  const handleDatabaseNext = () => setStep("provider");

  const handleProviderNext = async () => {
    try {
      await completeWizard();
    } catch {
      // non-critical
    }
    setStep("completed");
  };

  const handleFinish = () => {
    onComplete();
  };

  const handleSkip = async () => {
    try {
      await completeWizard();
    } catch {
      // non-critical
    }
    onComplete();
  };

  return (
    <div className="wizard-overlay">
      <div className="wizard-card">
        <div className="wizard-steps">
          <span className={`wizard-step-indicator${step === "language" ? " active" : ""}${step !== "language" ? " done" : ""}`}>
            {t("wizard.step.language")}
          </span>
          <span className="wizard-step-connector" />
          <span className={`wizard-step-indicator${step === "database" ? " active" : ""}${step !== "language" && step !== "database" ? " done" : ""}`}>
            {t("wizard.step.database")}
          </span>
          <span className="wizard-step-connector" />
          <span className={`wizard-step-indicator${step === "provider" ? " active" : ""}${step === "completed" ? " done" : ""}`}>
            {t("wizard.step.provider")}
          </span>
        </div>

        <div className="wizard-body">
          {step === "language" && (
            <div className="wizard-step-content">
              <h2>{t("wizard.title")}</h2>
              <p className="wizard-desc">{t("wizard.subtitle")}</p>
              <div className="wizard-field">
                <label>{t("language.label")}</label>
                <p className="wizard-field-desc">{t("wizard.languageDesc")}</p>
                <select
                  value={language}
                  onChange={(e) => setLanguage(e.target.value as Language)}
                >
                  <option value="en">{t("language.en")}</option>
                  <option value="zh">{t("language.zh")}</option>
                </select>
              </div>
            </div>
          )}

          {step === "database" && (
            <div className="wizard-step-content">
              <h2>{t("wizard.step.database")}</h2>
              <p className="wizard-desc">{t("wizard.databaseDesc")}</p>
              {dbError ? <div className="notice error">{dbError}</div> : null}
              {dbDone ? (
                <div className="wizard-success">{t("wizard.databaseDone")}</div>
              ) : (
                <button type="button" onClick={handleInitDb} disabled={isPending}>
                  {isPending ? "..." : t("wizard.databaseInit")}
                </button>
              )}
            </div>
          )}

          {step === "provider" && (
            <div className="wizard-step-content">
              <h2>{t("wizard.step.provider")}</h2>
              <p className="wizard-desc">{t("wizard.providerDesc")}</p>
              <p className="wizard-note">{t("wizard.completed")}</p>
            </div>
          )}

          {step === "completed" && (
            <div className="wizard-step-content">
              <h2>{t("wizard.completed")}</h2>
            </div>
          )}
        </div>

        <div className="wizard-footer">
          {step === "language" && (
            <div className="wizard-footer-inner">
              <button type="button" className="secondary" onClick={handleSkip}>
                {t("wizard.skip")}
              </button>
              <button type="button" onClick={handleLanguageNext}>
                {t("wizard.next")}
              </button>
            </div>
          )}
          {step === "database" && (
            <div className="wizard-footer-inner">
              <button type="button" className="secondary" onClick={() => setStep("language")}>
                {t("wizard.back")}
              </button>
              {dbDone ? (
                <button type="button" onClick={handleDatabaseNext}>
                  {t("wizard.next")}
                </button>
              ) : (
                <button type="button" className="secondary" onClick={handleDatabaseNext}>
                  {t("wizard.databaseSkip")}
                </button>
              )}
            </div>
          )}
          {step === "provider" && (
            <div className="wizard-footer-inner">
              <button type="button" className="secondary" onClick={() => setStep("database")}>
                {t("wizard.back")}
              </button>
              <button type="button" onClick={handleProviderNext}>
                {t("wizard.finish")}
              </button>
            </div>
          )}
          {step === "completed" && (
            <div className="wizard-footer-inner">
              <button type="button" onClick={handleFinish}>
                {t("wizard.finish")}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
