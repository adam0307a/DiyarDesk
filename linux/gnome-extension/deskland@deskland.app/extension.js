import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

import {Extension, InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AltTab from 'resource:///org/gnome/shell/ui/altTab.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Workspace} from 'resource:///org/gnome/shell/ui/workspace.js';
import {BaseWorkspaceGroup} from 'resource:///org/gnome/shell/ui/workspaceAnimation.js';

const WALLPAPER_TITLE = 'DiyarDeskWallpaper';
const DBUS_NAME = 'org.deskland.GnomeShell';
const DBUS_PATH = '/org/deskland/GnomeShell';

const DBUS_IFACE_XML = `
<node>
  <interface name="org.deskland.GnomeShell">
    <method name="Ping">
      <arg type="s" direction="out" name="status"/>
    </method>
    <method name="ShouldPause">
      <arg type="b" direction="out" name="paused"/>
    </method>
    <method name="LowerWallpaper">
      <arg type="b" direction="out" name="ok"/>
    </method>
    <method name="ListWindows">
      <arg type="s" direction="out" name="windowsJson"/>
    </method>
  </interface>
</node>`;

function isWallpaperWindow(win) {
    if (!win)
        return false;
    try {
        return win.get_title() === WALLPAPER_TITLE;
    } catch {
        return false;
    }
}

export default class DiyarDeskExtension extends Extension {
    enable() {
        this._injectionManager = new InjectionManager();
        this._trackedWindows = new Map();
        this._signals = [];
        this._restackIdleId = 0;
        this._restacking = false;
        this._dbusImpl = null;
        this._dbusOwnerId = 0;

        this._installOverrides();
        this._connectShellSignals();
        this._scanExistingWindows();
        this._exportDBus();
    }

    disable() {
        if (this._restackIdleId) {
            GLib.source_remove(this._restackIdleId);
            this._restackIdleId = 0;
        }

        for (const [win] of this._trackedWindows)
            win.disconnectObject(this);
        this._trackedWindows.clear();

        for (const {obj, id} of this._signals)
            obj.disconnect(id);
        this._signals = [];

        this._injectionManager?.clear();
        this._injectionManager = null;

        this._unexportDBus();
    }

    // ------------------------------------------------------------------ D-Bus
    _exportDBus() {
        this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE_XML, this);
        this._dbusOwnerId = Gio.bus_own_name(
            Gio.BusType.SESSION,
            DBUS_NAME,
            Gio.BusNameOwnerFlags.REPLACE,
            conn => {
                try {
                    this._dbusImpl?.export(conn, DBUS_PATH);
                } catch (err) {
                    logError(err, 'DiyarDesk: failed to export D-Bus interface');
                }
            },
            null,
            null
        );
    }

    _unexportDBus() {
        if (this._dbusImpl) {
            try {
                this._dbusImpl.unexport();
            } catch {
                // ignore
            }
            this._dbusImpl = null;
        }
        if (this._dbusOwnerId) {
            Gio.bus_unown_name(this._dbusOwnerId);
            this._dbusOwnerId = 0;
        }
    }

    Ping() {
        return 'ok';
    }

    ShouldPause() {
        const workspace = global.workspace_manager.get_active_workspace();
        if (!workspace)
            return false;
        const primary = Main.layoutManager.primaryIndex;
        const mon = Main.layoutManager.primaryMonitor;
        for (const win of workspace.list_windows()) {
            if (!win || win.minimized || isWallpaperWindow(win))
                continue;
            const wtype = win.get_window_type();
            if (wtype === Meta.WindowType.DESKTOP || wtype === Meta.WindowType.DOCK)
                continue;
            if (win.get_monitor() !== primary || !win.showing_on_its_workspace())
                continue;
            if (win.is_fullscreen() || win.is_maximized?.())
                return true;
            if (mon) {
                const rect = win.get_frame_rect();
                if (rect.width >= mon.width - 4 && rect.height >= mon.height - 40)
                    return true;
            }
        }
        return false;
    }

    LowerWallpaper() {
        let found = false;
        for (const win of this._trackedWindows.keys()) {
            this._placeWallpaperWindow(win);
            found = true;
        }
        return found;
    }

    ListWindows() {
        const workspace = global.workspace_manager.get_active_workspace();
        if (!workspace)
            return '[]';
        const primary = Main.layoutManager.primaryIndex;
        const out = [];
        for (const win of workspace.list_windows()) {
            if (!win || win.minimized || isWallpaperWindow(win))
                continue;
            const wtype = win.get_window_type();
            if (
                wtype === Meta.WindowType.DESKTOP ||
                wtype === Meta.WindowType.DOCK ||
                wtype === Meta.WindowType.SPLASHSCREEN ||
                wtype === Meta.WindowType.TOOLTIP ||
                wtype === Meta.WindowType.NOTIFICATION
            ) {
                continue;
            }
            if (win.is_override_redirect?.() || win.is_fullscreen?.())
                continue;
            if (win.get_monitor() !== primary || !win.showing_on_its_workspace())
                continue;
            const rect = win.get_frame_rect();
            if (rect.width < 60 || rect.height < 40)
                continue;
            const wid = typeof win.get_id === 'function' ? Number(win.get_id()) : out.length + 1;
            const cls = (win.get_wm_class?.() || win.get_title?.() || '').toString();
            out.push([
                wid & 0x7fffffff,
                Math.round(rect.x),
                Math.round(rect.y),
                Math.round(rect.x + rect.width),
                Math.round(rect.y + rect.height),
                cls,
            ]);
        }
        return JSON.stringify(out);
    }

    // -------------------------------------------------------------- overrides
    _installOverrides() {
        this._injectionManager.overrideMethod(
            Workspace.prototype,
            '_isOverviewWindow',
            original => function (window) {
                if (isWallpaperWindow(window))
                    return false;
                return original.call(this, window);
            }
        );

        this._injectionManager.overrideMethod(
            Meta.Display.prototype,
            'get_tab_list',
            original => function (type, workspace) {
                return original.call(this, type, workspace).filter(w => !isWallpaperWindow(w));
            }
        );

        if (AltTab.GroupCyclerPopup?.prototype?._getWindows) {
            this._injectionManager.overrideMethod(
                AltTab.GroupCyclerPopup.prototype,
                '_getWindows',
                original => function () {
                    return original.call(this).filter(w => !isWallpaperWindow(w));
                }
            );
        }

        if (Main.wm?._shouldAnimateActor) {
            this._injectionManager.overrideMethod(
                Main.wm,
                '_shouldAnimateActor',
                original => function (actor, types) {
                    if (isWallpaperWindow(actor?.meta_window))
                        return false;
                    return original.call(this, actor, types);
                }
            );
        }

        if (BaseWorkspaceGroup?.prototype?._isDesktopWindow) {
            this._injectionManager.overrideMethod(
                BaseWorkspaceGroup.prototype,
                '_isDesktopWindow',
                original => function (metaWindow) {
                    if (isWallpaperWindow(metaWindow))
                        return true;
                    return original.call(this, metaWindow);
                }
            );
        }
    }

    // ------------------------------------------------------- window stacking
    _connectShellSignals() {
        this._signals.push({
            obj: global.display,
            id: global.display.connect('window-created', (_d, win) => this._maybeTrack(win)),
        });
        this._signals.push({
            obj: global.display,
            id: global.display.connect('restacked', () => {
                this._lowerAllActorsSync();
                this._queueRestack();
            }),
        });
        this._signals.push({
            obj: global.display,
            id: global.display.connect('notify::focus-window', () => {
                const focused = global.display.get_focus_window();
                if (isWallpaperWindow(focused)) {
                    this._placeWallpaperWindow(focused);
                    this._queueRestack();
                }
            }),
        });
        this._signals.push({
            obj: global.workspace_manager,
            id: global.workspace_manager.connect('active-workspace-changed', () => {
                this._lowerAllSync();
                this._queueRestack();
            }),
        });
        this._signals.push({
            obj: global.window_manager,
            id: global.window_manager.connect('switch-workspace', () => {
                this._lowerAllSync();
                this._queueRestack();
            }),
        });
        this._signals.push({
            obj: Main.layoutManager,
            id: Main.layoutManager.connect('monitors-changed', () => this._queueRestack()),
        });
        this._signals.push({
            obj: Main.overview,
            id: Main.overview.connect('hiding', () => this._lowerAllActorsSync()),
        });
        this._signals.push({
            obj: Main.overview,
            id: Main.overview.connect('hidden', () => {
                this._lowerAllSync();
                this._queueRestack();
            }),
        });
    }

    _scanExistingWindows() {
        for (const actor of global.get_window_actors())
            this._maybeTrack(actor.meta_window);
    }

    _maybeTrack(win) {
        if (!win)
            return;
        if (isWallpaperWindow(win)) {
            this._trackWallpaperWindow(win);
            return;
        }
        // Wayland xdg_toplevel titles can arrive just after window-created.
        win.connectObject('notify::title', () => {
            if (isWallpaperWindow(win)) {
                win.disconnectObject(this);
                this._trackWallpaperWindow(win);
            }
        }, 'unmanaged', () => {
            win.disconnectObject(this);
        }, this);
    }

    _trackWallpaperWindow(win) {
        if (this._trackedWindows.has(win)) {
            this._placeWallpaperWindow(win);
            return;
        }
        this._trackedWindows.set(win, true);

        win.connectObject(
            'shown', () => {
                this._placeWallpaperWindow(win);
                this._queueRestack();
            },
            'raised', () => {
                this._placeWallpaperWindow(win);
                this._queueRestack();
            },
            'notify::appears-focused', () => {
                if (win.appears_focused) {
                    this._placeWallpaperWindow(win);
                    this._queueRestack();
                }
            },
            'notify::minimized', () => {
                if (win.minimized)
                    win.unminimize();
                this._placeWallpaperWindow(win);
                this._queueRestack();
            },
            'unmanaged', () => {
                win.disconnectObject(this);
                this._trackedWindows.delete(win);
            },
            this
        );

        this._placeWallpaperWindow(win);
        this._queueRestack();
    }

    _lowerActorSync(win) {
        if (!win)
            return;
        try {
            const actor = win.get_compositor_private();
            const bgGroup = Main.layoutManager._backgroundGroup;
            if (actor) {
                const parent = actor.get_parent();
                if (parent && bgGroup && parent === bgGroup.get_parent())
                    parent.set_child_above_sibling(actor, bgGroup);
                else if (parent)
                    parent.set_child_below_sibling(actor, null);
            }
        } catch {
            // ignore transient compositor state
        }
    }

    _lowerAllActorsSync() {
        for (const win of this._trackedWindows.keys())
            this._lowerActorSync(win);
    }

    _lowerAllSync() {
        for (const win of this._trackedWindows.keys())
            this._placeWallpaperWindow(win);
    }

    _queueRestack() {
        if (this._trackedWindows.size === 0 || this._restackIdleId)
            return;
        this._restackIdleId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._restackIdleId = 0;
            for (const win of this._trackedWindows.keys())
                this._placeWallpaperWindow(win);
            return GLib.SOURCE_REMOVE;
        });
    }

    _placeWallpaperWindow(win) {
        if (this._restacking || !win)
            return;
        this._restacking = true;
        try {
            if (!win.on_all_workspaces)
                win.stick();
            if (win.is_fullscreen?.())
                win.unfullscreen();
            if (win.minimized)
                win.unminimize();

            const mon = Main.layoutManager.primaryMonitor;
            const actor = win.get_compositor_private();
            if (mon && actor) {
                const rect = win.get_frame_rect();
                if (
                    rect.x !== mon.x ||
                    rect.y !== mon.y ||
                    rect.width !== mon.width ||
                    rect.height !== mon.height
                ) {
                    win.move_resize_frame(false, mon.x, mon.y, mon.width, mon.height);
                }
            }

            win.lower();
            this._lowerActorSync(win);
        } catch (err) {
            logError(err, 'DiyarDesk: failed to position wallpaper window');
        } finally {
            this._restacking = false;
        }
    }
}
