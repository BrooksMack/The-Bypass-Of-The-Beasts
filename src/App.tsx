import { useCallback, useEffect, useState } from "react";
import { errorMessage, getApi, type Api } from "./lib/api";
import { STAGES, type AppInfo, type HostReport, type SetupState, type Stage } from "./lib/types";
import { Welcome } from "./screens/Welcome";
import { CheckComputer } from "./screens/CheckComputer";
import { ChooseSetup } from "./screens/ChooseSetup";
import { ObtainFiles } from "./screens/ObtainFiles";
import { InstallDependencies } from "./screens/InstallDependencies";
import { CreateVm } from "./screens/CreateVm";
import { InstallWindows } from "./screens/InstallWindows";
import { FinishVerify } from "./screens/FinishVerify";
import { Dashboard } from "./screens/Dashboard";

const ORDER: Stage[] = STAGES.map((s) => s.id);

export default function App() {
  const [api, setApi] = useState<Api | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [state, setState] = useState<SetupState | null>(null);
  const [report, setReport] = useState<HostReport | null>(null);
  const [view, setView] = useState<Stage>("welcome");
  const [fatal, setFatal] = useState<string | null>(null);

  const refresh = useCallback(
    async (withHost?: boolean) => {
      if (!api) return;
      setState(await api.getState());
      if (withHost) setReport(await api.inspectHost());
    },
    [api],
  );

  useEffect(() => {
    getApi()
      .then(async (a) => {
        setApi(a);
        const [i, s] = await Promise.all([a.getAppInfo(), a.getState()]);
        setInfo(i);
        setState(s);
        setView(s.stage === "welcome" ? "welcome" : "welcome");
      })
      .catch((e) => setFatal(errorMessage(e)));
  }, []);

  const goTo = useCallback(
    async (stage: Stage) => {
      if (!api) return;
      setView(stage);
      // Persist forward progress only; going back to review does not rewind the saved stage.
      const cur = state?.stage ?? "welcome";
      if (ORDER.indexOf(stage) > ORDER.indexOf(cur)) {
        try {
          setState(await api.setStage(stage));
        } catch (e) {
          setFatal(errorMessage(e));
        }
      }
    },
    [api, state],
  );

  // Refresh the host report whenever we enter a stage that depends on it.
  useEffect(() => {
    if (!api || view === "welcome") return;
    if (!report || view === "check_computer" || view === "install_dependencies" || view === "obtain_files") {
      api.inspectHost().then(setReport).catch((e) => setFatal(errorMessage(e)));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, view]);

  if (fatal) {
    return (
      <main className="main">
        <h1>VM Setup Assistant could not start</h1>
        <div className="error-box">{fatal}</div>
      </main>
    );
  }
  if (!api || !state || !info) {
    return (
      <main className="main">
        <p aria-live="polite">Starting…</p>
      </main>
    );
  }

  const resumeStage = (): Stage => {
    // Mirror of SetupState::resume_stage on the Rust side, using durable facts.
    if (state.vm) {
      if (state.guest.windows_installed === "yes") return ORDER.indexOf(state.stage) >= ORDER.indexOf("dashboard") ? "dashboard" : "finish_and_verify";
      return "install_windows";
    }
    if (ORDER.indexOf(state.stage) >= ORDER.indexOf("create_vm")) return "create_vm";
    return state.stage === "welcome" ? "check_computer" : state.stage;
  };

  const needReport = view !== "welcome" && !report;
  const next = (s: Stage) => () => goTo(s);
  const visited = (s: Stage) => ORDER.indexOf(s) < ORDER.indexOf(state.stage) || (state.vm != null && ORDER.indexOf(s) <= ORDER.indexOf("create_vm"));

  return (
    <>
      {api.isMock && (
        <div className="mock-banner" role="note">
          MOCK BACKEND: this is a simulated backend for development and tests. Nothing here touches VirtualBox.
        </div>
      )}
      {info.startup_warning && (
        <div className="mock-banner" role="alert">
          {info.startup_warning}{" "}
          <button className="link" onClick={() => api.acknowledgeStartupWarning().then(() => setInfo({ ...info, startup_warning: null }))}>
            Dismiss
          </button>
        </div>
      )}
      <div className="app">
        <nav className="sidebar" aria-label="Setup steps">
          <div className="brand">VM Setup Assistant</div>
          <ol>
            {STAGES.map((s, i) => {
              const reachable = visited(s.id) || s.id === view || (state.vm != null && s.id === "dashboard");
              return (
                <li key={s.id} aria-current={view === s.id ? "step" : undefined} className={visited(s.id) && view !== s.id ? "done" : ""}>
                  <span className="step-num" aria-hidden>
                    {i + 1}
                  </span>
                  {reachable && s.id !== view ? (
                    <button className="link" onClick={() => setView(s.id)}>
                      {s.label}
                    </button>
                  ) : (
                    <span>{s.label}</span>
                  )}
                </li>
              );
            })}
          </ol>
          <div className="footer">
            Version {info.version}
            <br />
            <button className="link" onClick={() => api.openOfficialPage("project")}>
              Project page and documentation
            </button>
          </div>
        </nav>
        <main className="main">
          {view === "welcome" && (
            <Welcome
              hasProgress={state.stage !== "welcome"}
              onResume={() => goTo(resumeStage())}
              onStart={async () => {
                if (state.stage !== "welcome") {
                  if (!window.confirm("Start over? Your existing VM (if any) is kept in VirtualBox and can be managed there.")) return;
                  setState(await api.forgetSetup());
                  setReport(null);
                }
                goTo("check_computer");
              }}
            />
          )}
          {view !== "welcome" && needReport && <p aria-live="polite">Looking at this computer…</p>}
          {view === "check_computer" && <CheckComputer api={api} report={report} setReport={setReport} onNext={next("choose_setup")} />}
          {view === "choose_setup" && report && (
            <ChooseSetup
              api={api}
              report={report}
              state={state}
              onSaved={async () => {
                await refresh();
                goTo("obtain_files");
              }}
            />
          )}
          {view === "obtain_files" && report && <ObtainFiles api={api} report={report} state={state} refresh={refresh} onNext={next(report.virtualbox && report.virtualbox_version_ok !== false ? "create_vm" : "install_dependencies")} />}
          {view === "install_dependencies" && report && <InstallDependencies api={api} report={report} state={state} refresh={refresh} onNext={next("create_vm")} />}
          {view === "create_vm" && report && <CreateVm api={api} report={report} state={state} refresh={refresh} onNext={next("install_windows")} />}
          {view === "install_windows" && report && <InstallWindows api={api} report={report} state={state} refresh={refresh} onNext={next("finish_and_verify")} />}
          {view === "finish_and_verify" && report && <FinishVerify api={api} report={report} state={state} refresh={refresh} onNext={next("dashboard")} />}
          {view === "dashboard" && report && <Dashboard api={api} report={report} state={state} refresh={refresh} onGoTo={(s) => setView(s)} />}
        </main>
      </div>
    </>
  );
}
