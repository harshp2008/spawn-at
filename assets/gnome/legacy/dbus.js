/**
 * spawn-at — D-Bus interface definition and session export (Legacy GNOME 42-44).
 */

const { Gio } = imports.gi;

var DBUS_IFACE = `
<node>
  <interface name="org.gnome.Shell.Extensions.SpawnAt">
    <property name="ProtocolVersion" type="u" access="read"/>
    <method name="ArmSpawn">
      <arg type="s" name="target_id" direction="in"/>
      <arg type="s" name="instructions_json" direction="in"/>
    </method>
    <method name="DisarmSpawn">
      <arg type="s" name="target_id" direction="in"/>
      <arg type="b" name="disarmed" direction="out"/>
    </method>
    <method name="ExecuteBatch">
      <arg type="s" name="target_id" direction="in"/>
      <arg type="s" name="instructions_json" direction="in"/>
    </method>
    <method name="GetCursor">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
    <method name="GetPointer">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
    <method name="GetWorkareas">
      <arg type="s" name="json_layout" direction="out"/>
    </method>
    <method name="GetLayout">
      <arg type="s" name="json_layout" direction="out"/>
    </method>
    <method name="GetWindows">
      <arg type="s" name="json_windows" direction="out"/>
    </method>
    <method name="MoveWindow">
      <arg type="s" name="app_id" direction="in"/>
      <arg type="i" name="x" direction="in"/>
      <arg type="i" name="y" direction="in"/>
    </method>
    <method name="FocusWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="DefocusWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="s" name="mode" direction="in"/>
      <arg type="s" name="destination" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="SetWindowState">
      <arg type="s" name="target" direction="in"/>
      <arg type="s" name="state" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="CloseWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="SetLogging">
      <arg type="b" name="enabled" direction="in"/>
    </method>
    <signal name="WorkareaChanged"/>
    <signal name="SpawnClaimed">
      <arg type="s" name="target_id"/>
      <arg type="b" name="success"/>
      <arg type="t" name="window_id"/>
      <arg type="i" name="x"/>
      <arg type="i" name="y"/>
      <arg type="u" name="w"/>
      <arg type="u" name="h"/>
      <arg type="b" name="size_raised"/>
      <arg type="s" name="error"/>
    </signal>
  </interface>
</node>`;

var DBusManager = class DBusManager {
    constructor(target) {
        this._target = target;
        this._dbusImpl = null;
    }

    export(bus = Gio.DBus.session, path = '/org/gnome/Shell/Extensions/SpawnAt') {
        try {
            this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE, this._target);
            this._dbusImpl.export(bus, path);
        } catch (e) {
            log(`[SpawnAt] Failed to export D-Bus interface: ${e}`);
        }
    }

    emitSignal(name, params = null) {
        if (this._dbusImpl) {
            this._dbusImpl.emit_signal(name, params);
        }
    }

    unexport() {
        if (this._dbusImpl) {
            try { this._dbusImpl.unexport(); } catch (_e) {}
            this._dbusImpl = null;
        }
    }
};

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { DBUS_IFACE, DBusManager };
}
