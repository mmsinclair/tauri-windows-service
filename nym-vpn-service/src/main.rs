mod daemon;
mod install;

#[macro_use]
extern crate windows_service;

use crate::daemon::DaemonState;
use log::info;
use once_cell::sync::Lazy;
use std::env;
use std::ffi::OsString;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use windows_service::service::{
    Service, ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept,
    ServiceDependency, ServiceErrorControl, ServiceExitCode, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceSidType, ServiceStartType, ServiceState,
    ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

define_windows_service!(ffi_service_main, my_service_main);

pub(crate) static SERVICE_NAME: &str = "nymvpn_daemon";
pub(crate) static SERVICE_DISPLAY_NAME: &str = "NymVPN Service";

pub(crate) static SERVICE_DESCRIPTION: &str =
    "A service that creates and runs tunnels to the Nym network";
static SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

static SERVICE_ACCESS: Lazy<ServiceAccess> = Lazy::new(|| {
    ServiceAccess::QUERY_CONFIG
        | ServiceAccess::CHANGE_CONFIG
        | ServiceAccess::START
        | ServiceAccess::DELETE
});

const SERVICE_RECOVERY_LAST_RESTART_DELAY: Duration = Duration::from_secs(60 * 10);
const SERVICE_FAILURE_RESET_PERIOD: Duration = Duration::from_secs(60 * 15);

fn my_service_main(arguments: Vec<OsString>) {
    if let Err(_e) = run_service(arguments) {
        // Handle error in some way.
    }
}

fn run_service(_arguments: Vec<OsString>) -> windows_service::Result<()> {
    info!("Creating tokio runtime...");

    let rt = Arc::new(Runtime::new().unwrap());
    let (tx, rx): (
        Sender<crate::daemon::DaemonState>,
        Receiver<crate::daemon::DaemonState>,
    ) = mpsc::channel();
    let daemon = Arc::new(Mutex::new(daemon::Daemon::new(tx.clone())));

    let rt_handler = rt.clone();
    let daemon_handler = daemon.clone();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                rt_handler.block_on(async {
                    let mut guard = daemon_handler.lock().await;
                    guard.stop().await;
                });
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => {
                rt_handler.block_on(async {
                    let guard = daemon_handler.lock().await;
                    let status = guard.get_status().await;
                    info!("Status is {:#?}", status)
                });
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    let next_status = ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };

    info!("Service is starting...");
    rt.block_on(async {
        let mut guard = daemon.lock().await;
        guard.start().await;
    });

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    info!("Service has started");

    let mut state = DaemonState::Running;
    while state != DaemonState::Stopped {
        rt.block_on(async {
            state = rx.recv().unwrap();
        });
    }

    info!("Service is stopping!");

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("Service has stopped!");

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("Unable to connect to service manager")]
    ConnectServiceManager(#[source] windows_service::Error),

    #[error("Unable to create a service")]
    CreateService(#[source] windows_service::Error),
}

pub fn install_service() -> Result<(), InstallError> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .map_err(InstallError::ConnectServiceManager)?;

    let service = service_manager
        .create_service(&get_service_info(), *SERVICE_ACCESS)
        .or(open_update_service(&service_manager))
        .map_err(InstallError::CreateService)?;

    let recovery_actions = vec![
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(3),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(30),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: SERVICE_RECOVERY_LAST_RESTART_DELAY,
        },
    ];

    let failure_actions = ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(SERVICE_FAILURE_RESET_PERIOD),
        reboot_msg: None,
        command: None,
        actions: Some(recovery_actions),
    };

    service
        .update_failure_actions(failure_actions)
        .map_err(InstallError::CreateService)?;
    service
        .set_failure_actions_on_non_crash_failures(true)
        .map_err(InstallError::CreateService)?;

    // Change how the service SID is added to the service process token.
    // WireGuard needs this.
    service
        .set_config_service_sid_info(ServiceSidType::Unrestricted)
        .map_err(InstallError::CreateService)?;

    Ok(())
}

fn open_update_service(
    service_manager: &ServiceManager,
) -> Result<Service, windows_service::Error> {
    let service = service_manager.open_service(SERVICE_NAME, *SERVICE_ACCESS)?;
    service.change_config(&get_service_info())?;
    Ok(service)
}

pub(crate) fn get_service_info() -> ServiceInfo {
    ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: env::current_exe().unwrap(),
        launch_arguments: vec![OsString::from("--run-as-service"), OsString::from("-v")],
        dependencies: vec![
            // Base Filter Engine
            ServiceDependency::Service(OsString::from("BFE")),
            // Network Store Interface Service
            // This service delivers network notifications (e.g. interface addition/deleting etc).
            ServiceDependency::Service(OsString::from("NSI")),
        ],
        account_name: None, // run as System
        account_password: None,
    }
}

fn main() -> Result<(), windows_service::Error> {
    // crate::install::install_service()?;

    println!("Setup complete");

    // Register generated `ffi_service_main` with the system and start the service, blocking
    // this thread until the service is stopped.
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}
