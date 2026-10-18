// prevents an extra console window on windows in release, do not remove
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pelican_mods_lib::run()
}
