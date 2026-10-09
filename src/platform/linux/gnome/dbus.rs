use zbus::proxy;

#[proxy(
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/SpawnAt",
    interface = "org.gnome.Shell.Extensions.SpawnAt"
)]
pub trait SpawnAt {
    #[zbus(property)]
    fn protocol_version(&self) -> zbus::Result<u32>;
    fn arm_spawn(&self, target_id: &str, instructions_json: &str) -> zbus::Result<()>;
    fn disarm_spawn(&self, target_id: &str) -> zbus::Result<bool>;
    fn execute_batch(&self, target_id: &str, instructions_json: &str) -> zbus::Result<()>;
    fn get_cursor(&self) -> zbus::Result<(i32, i32)>;
    fn get_pointer(&self) -> zbus::Result<(i32, i32)>;
    fn get_workareas(&self) -> zbus::Result<String>;
    fn get_layout(&self) -> zbus::Result<String>;
    fn get_windows(&self) -> zbus::Result<String>;
    fn move_window(&self, app_id: &str, x: i32, y: i32) -> zbus::Result<()>;
    fn focus_window(&self, target: &str) -> zbus::Result<bool>;
    fn defocus_window(&self, target: &str, mode: &str, destination: &str) -> zbus::Result<bool>;
    fn set_window_state(&self, target: &str, state: &str) -> zbus::Result<bool>;
    fn close_window(&self, target: &str) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn workarea_changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn spawn_claimed(
        &self,
        target_id: &str,
        success: bool,
        window_id: u64,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        size_raised: bool,
        error: &str,
    ) -> zbus::Result<()>;
}
