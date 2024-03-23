use crate::{get_service_info, SERVICE_DESCRIPTION, SERVICE_NAME};
use log::info;
use windows_service::{
    service::ServiceAccess,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub(crate) fn install_service() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    println!(
        "Registering event logger {}...",
        crate::SERVICE_DISPLAY_NAME
    );
    eventlog::register(crate::SERVICE_DISPLAY_NAME).unwrap();
    println!("Starting logging...");
    eventlog::init(crate::SERVICE_DISPLAY_NAME, log::Level::Info).unwrap();

    info!("NymVPN logging enabled");

    println!("Service registration...");
    if service_manager
        .open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_err()
    {
        println!("Service not registered...");
        let service_info = get_service_info();
        let service =
            service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
        service.set_description(SERVICE_DESCRIPTION)?;
    }
    Ok(())
}
