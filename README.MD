# Explorer Software for the Data of the COCoNuT experiment

This Application is written in Rust and the GUI is based on egui.  
The source code can be run using ```cargo run``` or bundled to a platform-executable using ```cargo bundle```.  
Currently [cargo bundle](https://github.com/burtonageo/cargo-bundle) only supports linux and macOS bundles [see github issue](https://github.com/burtonageo/cargo-bundle/issues/77).
As a work-around we can use [cargo wix](https://github.com/volks73/cargo-wix) to create a windows installer.  
It can be compiled and run on all platforms.
Tested on:
- MacOS 12.4 Monterey x86
- MacOS 10.14.6 Mojave x86
- Debian 12 (Testing) x86
- Windows 10 x86

The application is an improved version of and replaces the original Toptica LabView-based control software.

On Debian 12 (Testing) the following error occurred:
```
Error: glib-2.0 was not found in the pkg-config search path.
```
solved through
```
sudo apt-get install libgtk-3-dev
```

One might have to delete the ```Cargo.lock``` file before compiling.  
