//! sysd1stub: answers the few org.freedesktop.systemd1.Manager calls the GNOME Shell makes when it
//! starts X11 services, on the private session bus (DBUS_SESSION_BUS_ADDRESS). It starts nothing.
//! `StartUnit` returns a job path and, 50 ms later, emits `JobRemoved(.., "done")`; `GetUnit` says
//! the unit is not loaded. Run only through psh-start.sh.

use std::time::Duration;
use zbus::object_server::SignalContext;
use zbus::zvariant::ObjectPath;

struct Manager;

#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl Manager {
    async fn get_unit(&self, name: &str) -> zbus::fdo::Result<ObjectPath<'static>> {
        Err(zbus::fdo::Error::Failed(format!("NoSuchUnit: unit {name} not loaded")))
    }

    async fn subscribe(&self) {}

    async fn start_unit(
        &self,
        name: &str,
        _mode: &str,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<ObjectPath<'static>> {
        let unit = name.to_string();
        let ctxt = ctxt.into_owned();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let job = ObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap();
            let _ = zbus::block_on(Manager::job_removed(&ctxt, 1, job, &unit, "done"));
        });
        Ok(ObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap())
    }

    #[zbus(signal)]
    async fn job_removed(ctxt: &SignalContext<'_>, id: u32, job: ObjectPath<'_>, unit: &str, result: &str) -> zbus::Result<()>;
}

fn main() -> zbus::Result<()> {
    let _conn = zbus::blocking::connection::Builder::session()?
        .name("org.freedesktop.systemd1")?
        .serve_at("/org/freedesktop/systemd1", Manager)?
        .build()?;
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
