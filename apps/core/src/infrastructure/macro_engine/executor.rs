use std::{
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use dashmap::DashMap;
use deno_core::{v8, JsRuntime, ModuleSpecifier, RuntimeOptions};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info};

use crate::{
    application::state::AppState,
    domain::{event::Event, instance::InstanceId},
    infrastructure::macro_engine::{
        loader::TypescriptModuleLoader,
        ops::{
            events::{amberite_events, EventRx},
            instance_control::amberite_instance_control,
            prelude::amberite_prelude,
        },
    },
};

pub type MacroPid = u64;

/// Manages running macro JS/TS processes.
pub struct MacroExecutor {
    processes: Arc<DashMap<MacroPid, v8::IsolateHandle>>,
    next_pid: Arc<AtomicU64>,
}

impl MacroExecutor {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(DashMap::new()),
            next_pid: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Spawn a macro from `path` for `instance_id`. Returns the macro PID.
    pub fn spawn_macro(
        &self,
        instance_id: InstanceId,
        macro_path: PathBuf,
        state: Arc<AppState>,
        event_rx: broadcast::Receiver<Event>,
    ) -> MacroPid {
        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);
        let processes = Arc::clone(&self.processes);
        let rt_handle = tokio::runtime::Handle::current();

        std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();
            rt_handle.block_on(local.run_until(async move {
                let rx_arc: EventRx = Arc::new(Mutex::new(event_rx));
                let state_clone = Arc::clone(&state);
                let id_clone = instance_id.clone();
                let rx_for_state = Arc::clone(&rx_arc);
                let pid_val = pid;

                // Attach op_state initializer to the prelude extension (RuntimeOptions
                // no longer has op_state_fn in deno_core 0.354+).
                let mut prelude_ext = amberite_prelude::init();
                prelude_ext.op_state_fn = Some(Box::new(move |op_state| {
                    op_state.put(state_clone);
                    op_state.put(id_clone);
                    op_state.put(rx_for_state);
                    op_state.put(pid_val);
                }));

                let mut runtime = JsRuntime::new(RuntimeOptions {
                    module_loader: Some(Rc::new(TypescriptModuleLoader)),
                    extensions: vec![
                        prelude_ext,
                        amberite_events::init(),
                        amberite_instance_control::init(),
                    ],
                    ..Default::default()
                });

                // Register the isolate handle so kill_macro can terminate it.
                let iso_handle = runtime.v8_isolate().thread_safe_handle();
                processes.insert(pid, iso_handle);

                let url = format!("file://{}", macro_path.display());
                let result: Result<(), _> = (async {
                    let specifier = ModuleSpecifier::parse(&url)?;
                    let module_id = runtime.load_main_es_module(&specifier).await?;
                    let eval = runtime.mod_evaluate(module_id);
                    runtime.run_event_loop(Default::default()).await?;
                    eval.await?;
                    Ok::<_, deno_core::error::AnyError>(())
                }).await;

                if let Err(e) = result {
                    error!("Macro {pid} for {instance_id} errored: {e}");
                } else {
                    info!("Macro {pid} for {instance_id} completed");
                }
                processes.remove(&pid);
            }));
        });

        pid
    }

    /// Kill a running macro by PID. Returns true if found and killed.
    pub fn kill_macro(&self, pid: MacroPid) -> bool {
        if let Some((_, handle)) = self.processes.remove(&pid) {
            handle.terminate_execution();
            true
        } else {
            false
        }
    }

    /// List all running macro PIDs.
    pub fn list_pids(&self) -> Vec<MacroPid> {
        self.processes.iter().map(|e| *e.key()).collect()
    }
}

impl Default for MacroExecutor {
    fn default() -> Self {
        Self::new()
    }
}
