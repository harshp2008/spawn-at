import re

with open("src/platform/linux/gnome/mod.rs", "r") as f:
    content = f.read()

# Replace TargetGeometry with Instruction
content = content.replace("use crate::core::geometry::{Rect, TargetGeometry};", "use crate::core::geometry::{Rect, TargetGeometry};\nuse crate::core::types::Instruction;")

spawn_at_code = """    /// Primes the GNOME Shell extension via D-Bus and launches the command.
    async fn spawn_at(
        &self,
        app_id: &str,
        command: &[String],
        instructions: &[Instruction],
    ) -> Result<(), DriverError> {
        if command.is_empty() {
            return Err(DriverError::Execution(
                "Cannot spawn application: command vector is empty".into(),
            ));
        }

        let instructions_json = serde_json::to_string(instructions)
            .map_err(|e| DriverError::Execution(format!("Failed to serialize instructions: {}", e).into()))?;

        self.proxy
            .execute_batch(app_id, &instructions_json)
            .await
            .map_err(|e| {
                DriverError::IpcError(format!(
                    "Failed to communicate with SpawnAt GNOME extension via D-Bus: {}\n\
                     Reason: The extension does not appear to be running on the session bus.\n\
                     Fix: Run 'spawn-at install' to install and activate the extension.",
                    e
                ))
            })?;

        Command::new(&command[0])
            .args(&command[1..])
            .spawn()
            .map_err(|e| {
                DriverError::Execution(
                    format!("Failed to spawn command '{}': {}", command[0], e).into(),
                )
            })?;

        Ok(())
    }

    async fn execute_batch(&self, target_id: &str, instructions: &[Instruction]) -> Result<(), DriverError> {
        let instructions_json = serde_json::to_string(instructions)
            .map_err(|e| DriverError::Execution(format!("Failed to serialize instructions: {}", e).into()))?;

        self.proxy
            .execute_batch(target_id, &instructions_json)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        Ok(())
    }"""

old_spawn = re.search(r"/// Primes the GNOME Shell extension via D-Bus and launches the command.*?Ok\(\(\)\)\n    }", content, re.DOTALL)
if old_spawn:
    content = content[:old_spawn.start()] + spawn_at_code + content[old_spawn.end():]

with open("src/platform/linux/gnome/mod.rs", "w") as f:
    f.write(content)

