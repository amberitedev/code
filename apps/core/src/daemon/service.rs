//! Core as an OS service running as `copal`: a Windows Service, a systemd unit, or a launchd
//! daemon. The OS starts it at boot and starts it again whenever it exits. Stopping the service
//! stops Core only; shims and their servers keep running.

use std::path::Path;

use color_eyre::eyre::Result;

#[cfg(windows)]
pub use platform::dispatch;

/// The service's name on every OS.
pub const NAME: &str = "copal";

/// Register Core to run `exe` as `copal`, and start it. Replaces an earlier registration.
/// Returns a line to print, or why the service could not be registered.
pub fn register(exe: &Path, core_password: Option<&str>) -> Result<String> {
    platform::register(exe, core_password)
}

/// Stop the service if it is registered and running, waiting until it has stopped.
pub fn stop() -> Result<()> {
    platform::stop()
}

/// Start the service again after `stop`. Fails when it is not registered.
pub fn start() -> Result<()> {
    platform::start()
}

/// Stop and remove the service. A service that is not registered is skipped.
pub fn unregister() -> Result<()> {
    platform::unregister()
}

#[cfg(windows)]
mod platform {
    use std::{
        ffi::OsString,
        path::Path,
        sync::Arc,
        time::{Duration, Instant},
    };

    use color_eyre::eyre::{Result, WrapErr, eyre};
    use tokio::sync::Notify;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceAccess, ServiceAction, ServiceActionType, ServiceControl,
            ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
            ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo,
            ServiceStartType, ServiceState, ServiceStatus, ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    use windows_sys::Win32::{
        Security::DACL_SECURITY_INFORMATION,
        System::Services::SetServiceObjectSecurity,
    };

    use super::NAME;
    use crate::daemon::{
        accounts::CORE,
        win::{SecurityDescriptor, account_sid, check},
    };

    /// The default service permissions, plus start, stop, and query for `copal`, so the updater
    /// can restart Core.
    fn permissions(core_sid: &str) -> String {
        format!(
            "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)\
             (A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;CCLCSWRPWPLOCRRC;;;{core_sid})"
        )
    }

    pub fn register(exe: &Path, core_password: Option<&str>) -> Result<String> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT
                | ServiceManagerAccess::CREATE_SERVICE,
        )?;
        let info = ServiceInfo {
            name: NAME.into(),
            display_name: "Copal".into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe.into(),
            launch_arguments: vec!["run".into(), "--service".into()],
            dependencies: vec![],
            account_name: Some(format!(r".\{CORE}").into()),
            account_password: core_password.map(Into::into),
        };
        let access = ServiceAccess::QUERY_STATUS
            | ServiceAccess::START
            | ServiceAccess::CHANGE_CONFIG
            | ServiceAccess::WRITE_DAC;
        let service = match manager.open_service(NAME, access) {
            Ok(service) => {
                service.change_config(&info)?;
                service
            }
            Err(_) => manager.create_service(&info, access)?,
        };
        service.set_description(
            "Runs and manages Minecraft servers for Amberite.",
        )?;
        let restart = ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(5),
        };
        service.update_failure_actions(ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(
                Duration::from_secs(24 * 60 * 60),
            ),
            reboot_msg: None,
            command: None,
            actions: Some(vec![restart.clone(), restart.clone(), restart]),
        })?;
        // Restart after any exit with an error, not only after a crash.
        service.set_failure_actions_on_non_crash_failures(true)?;
        let descriptor =
            SecurityDescriptor::from_sddl(&permissions(&account_sid(CORE)?))?;
        // SAFETY: a valid service handle and descriptor.
        check(unsafe {
            SetServiceObjectSecurity(
                service.raw_handle(),
                DACL_SECURITY_INFORMATION,
                descriptor.as_ptr(),
            )
        })
        .wrap_err("could not let copal restart the service")?;
        service
            .start::<&str>(&[])
            .wrap_err("the service did not start")?;
        Ok(format!("Windows service \"{NAME}\" registered and started"))
    }

    pub fn stop() -> Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT,
        )?;
        let Ok(service) = manager.open_service(
            NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP,
        ) else {
            return Ok(());
        };
        if service.query_status()?.current_state != ServiceState::Stopped {
            let _ = service.stop();
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        while service.query_status()?.current_state != ServiceState::Stopped {
            if Instant::now() > deadline {
                return Err(eyre!("the {NAME} service did not stop"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    pub fn start() -> Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT,
        )?;
        manager
            .open_service(NAME, ServiceAccess::START)?
            .start::<&str>(&[])?;
        Ok(())
    }

    pub fn unregister() -> Result<()> {
        stop()?;
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT,
        )?;
        if let Ok(service) = manager.open_service(NAME, ServiceAccess::DELETE) {
            service.delete()?;
        }
        Ok(())
    }

    define_windows_service!(ffi_service_main, service_main);

    /// Run Core under the service manager. `copal run --service` lands here.
    pub fn dispatch() -> Result<()> {
        service_dispatcher::start(NAME, ffi_service_main)
            .wrap_err("copal run --service is only for the service manager")
    }

    fn service_main(_arguments: Vec<OsString>) {
        // A service has no console, so the file log starts before anything can fail.
        crate::daemon::logging(&crate::daemon::paths::Paths::new(
            crate::daemon::config::data_dir(),
        ));
        let stop = Arc::new(Notify::new());
        let stop_requested = Arc::clone(&stop);
        let Ok(status) = service_control_handler::register(
            NAME,
            move |control| match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    stop_requested.notify_one();
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => {
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            },
        ) else {
            return;
        };
        let report = |state, accept, exit_code| {
            let _ = status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accept,
                exit_code,
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            });
        };
        report(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            ServiceExitCode::Win32(0),
        );
        let result = tokio::runtime::Runtime::new()
            .map_err(Into::into)
            .and_then(|runtime| {
                runtime.block_on(crate::daemon::serve(async move {
                    stop.notified().await;
                }))
            });
        let exit_code = match result {
            Ok(()) => ServiceExitCode::Win32(0),
            Err(error) => {
                tracing::error!("{error:?}");
                ServiceExitCode::ServiceSpecific(1)
            }
        };
        report(
            ServiceState::Stopped,
            ServiceControlAccept::empty(),
            exit_code,
        );
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::Path;

    use color_eyre::eyre::Result;

    use super::NAME;
    use crate::daemon::accounts::CORE;

    const UNIT: &str = "/etc/systemd/system/copal.service";

    /// Whether systemd runs this machine. WSL without systemd and containers do not.
    fn systemd() -> bool {
        Path::new("/run/systemd/system").exists()
    }

    fn systemctl(args: &[&str]) -> Result<()> {
        let output = std::process::Command::new("systemctl")
            .args(args)
            .output()?;
        if !output.status.success() {
            color_eyre::eyre::bail!(
                "systemctl {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    pub fn register(
        exe: &Path,
        _core_password: Option<&str>,
    ) -> Result<String> {
        // KillMode=process: stopping or updating Core never touches the shims it started.
        let unit = format!(
            "[Unit]\n\
             Description=Copal, Amberite's Minecraft server manager\n\
             Wants=network-online.target\n\
             After=network-online.target\n\
             \n\
             [Service]\n\
             User={CORE}\n\
             Group={CORE}\n\
             ExecStart={} run\n\
             KillMode=process\n\
             Restart=always\n\
             RestartSec=5\n\
             UMask=0027\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n",
            exe.display()
        );
        std::fs::write(UNIT, unit)?;
        if !systemd() {
            return Ok(format!(
                "systemd is not running here, so Core does not start at boot. Run it with: sudo -u {CORE} {} run",
                exe.display()
            ));
        }
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", NAME])?;
        systemctl(&["restart", NAME])?;
        Ok(format!("systemd unit {NAME}.service enabled and started"))
    }

    pub fn stop() -> Result<()> {
        if systemd() && Path::new(UNIT).exists() {
            systemctl(&["stop", NAME])?;
        }
        Ok(())
    }

    pub fn start() -> Result<()> {
        systemctl(&["start", NAME])
    }

    pub fn unregister() -> Result<()> {
        if systemd() && Path::new(UNIT).exists() {
            systemctl(&["disable", "--now", NAME])?;
        }
        match std::fs::remove_file(UNIT) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
        if systemd() {
            systemctl(&["daemon-reload"])?;
        }
        Ok(())
    }
}

/// Written against `launchctl` and `launchd.plist(5)` as documented; not yet run on a Mac.
#[cfg(target_os = "macos")]
mod platform {
    use std::{path::Path, process::Command};

    use color_eyre::eyre::Result;

    use crate::daemon::accounts::CORE;

    const LABEL: &str = "dev.amberite.copal";
    const PLIST: &str = "/Library/LaunchDaemons/dev.amberite.copal.plist";

    pub fn register(
        exe: &Path,
        _core_password: Option<&str>,
    ) -> Result<String> {
        stop()?;
        // AbandonProcessGroup: launchd leaves the shims alone when Core exits.
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>UserName</key><string>{CORE}</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>run</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>AbandonProcessGroup</key><true/>
  <key>Umask</key><integer>23</integer>
</dict>
</plist>
"#,
            exe.display()
        );
        std::fs::write(PLIST, plist)?;
        start()?;
        Ok(format!("launchd daemon {LABEL} loaded"))
    }

    pub fn stop() -> Result<()> {
        // Fails when the daemon is not loaded, which is fine.
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("system/{LABEL}")])
            .output()?;
        Ok(())
    }

    pub fn start() -> Result<()> {
        let output = Command::new("launchctl")
            .args(["bootstrap", "system", PLIST])
            .output()?;
        if !output.status.success() {
            color_eyre::eyre::bail!(
                "launchctl bootstrap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    pub fn unregister() -> Result<()> {
        stop()?;
        match std::fs::remove_file(PLIST) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error.into())
            }
            _ => Ok(()),
        }
    }
}
